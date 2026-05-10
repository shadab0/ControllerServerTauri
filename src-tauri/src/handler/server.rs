use std::{any::Any, net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use tokio::{net::UdpSocket, sync::Notify};
use local_ip_address::local_ip;
use crate::{types::{ClientInfo, ListenerState, XinputGamepad, CLIENT_SLOTS, CONNECTED_CLIENTS, GLOBAL_LISTENER, MAX_CLIENTS, SERVER_IP}, vbus::driver::{ControllerDevice, VBusDriver}};

pub async fn start() -> String {
    let socket = Arc::new(
        UdpSocket::bind(format!("{}:40405", local_ip().unwrap()))
            .await
            .unwrap(),
    );

    {
        let mut ip = SERVER_IP.lock().await;
        let local_ip = local_ip().unwrap().to_string();
        ip.ip = local_ip;
        ip.port = "40405".to_owned();
    }

    let notifylistener = Arc::new(Notify::new());

    let listenerstate = ListenerState {
        notify: notifylistener.clone(),
    };

    {
        let mut server_lock = GLOBAL_LISTENER.lock().await;
        *server_lock = Some(listenerstate.clone());
    }

    let device = Arc::new(VBusDriver);

    // UDP Receive Loop
    tokio::spawn({
        let notify = notifylistener.clone();
        let socket = Arc::clone(&socket);
        let device = Arc::clone(&device);
        async move {
            let mut buf = [0u8; 1024];
            let mut device_buffer: [u8; 28] = [0; 28];
            device_buffer[0] = 0x1C;
            device_buffer[9] = 0x14;
            loop {
                tokio::select! {
                    res = socket.recv_from(&mut buf) => {
                        if let Ok((len, addr)) = res {
                            if len == 0 { continue; }
                            let packet_type = buf[0];

                            // packet_type: 0x00 = Connect, 0x01 = Data, 0x02 = Disconnect, 0x04 = Heartbeat
                            if packet_type == 0x00 {
                                if let Some((slot_idx, client_id)) = acquire_client_slot().await {
                                    println!("UDP Client {} connected from {}", client_id, addr);
                                    {
                                        let mut slots = CLIENT_SLOTS.lock().await;
                                        slots[slot_idx].addr = Some(addr);
                                        slots[slot_idx].last_packet_time = Some(Instant::now());
                                    }
                                    add_connected_client(addr, slot_idx).await;
                                    print!("{}", device.plug_in(client_id as u32));
                                    
                                    // Send assigned slot back to client (1-based index)
                                    let mut resp = [0u8; 2];
                                    resp[0] = 0x00; // Connect Ack
                                    resp[1] = client_id as u8;
                                    let _ = socket.send_to(&resp, addr).await;
                                }
                            } else if packet_type == 0x01 && len >= 22 {
                                let slot_idx = buf[1] as usize - 1; // Client slot is 1-based index from packet
                                if slot_idx < MAX_CLIENTS {
                                    let mut should_execute = false;
                                    {
                                        let mut slots = CLIENT_SLOTS.lock().await;
                                        let client_slot = &mut slots[slot_idx];
                                        
                                        if client_slot.occupied && client_slot.addr == Some(addr) {
                                            client_slot.last_packet_time = Some(Instant::now());
                                            
                                            let seq = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
                                            if seq > client_slot.last_sequence {
                                                client_slot.last_sequence = seq;
                                                should_execute = true;
                                            }
                                        }
                                    }

                                    if should_execute {
                                        let user_index = slot_idx;
                                        
                                        // Parse 16-byte state from buf[6..22]
                                        let w_buttons = u16::from_le_bytes([buf[6], buf[7]]);
                                        let lt = buf[8];
                                        let rt = buf[9];
                                        let lx = i16::from_le_bytes([buf[10], buf[11]]);
                                        let ly = i16::from_le_bytes([buf[12], buf[13]]);
                                        let rx = i16::from_le_bytes([buf[14], buf[15]]);
                                        let ry = i16::from_le_bytes([buf[16], buf[17]]);
                                        
                                        let gamepad = XinputGamepad {
                                            w_buttons,
                                            b_left_trigger: lt,
                                            b_right_trigger: rt,
                                            s_thumb_lx: lx,
                                            s_thumb_ly: ly,
                                            s_thumb_rx: rx,
                                            s_thumb_ry: ry,
                                        };
                                        
                                        let client_id = user_index + 1;
                                        device_buffer[4] = ((client_id >> 0) & 0xFF) as u8;
                                        device_buffer[5] = ((client_id >> 8) & 0xFF) as u8;
                                        device_buffer[6] = ((client_id >> 16) & 0xFF) as u8;
                                        device_buffer[7] = ((client_id >> 24) & 0xFF) as u8;
                                        
                                        device.execute(&gamepad, &mut device_buffer);
                                    }
                                }
                            } else if packet_type == 0x02 {
                                // Disconnect
                                let slot_idx = buf[1] as usize - 1;
                                if slot_idx < MAX_CLIENTS {
                                    let mut slots = CLIENT_SLOTS.lock().await;
                                    if slots[slot_idx].occupied && slots[slot_idx].addr == Some(addr) {
                                        println!("UDP Client disconnected: {}", addr);
                                        drop(slots);
                                        release_client_slot(slot_idx + 1, &device).await;
                                    }
                                }
                            } else if packet_type == 0x04 {
                                // Heartbeat
                                let slot_idx = buf[1] as usize - 1;
                                if slot_idx < MAX_CLIENTS {
                                    let mut slots = CLIENT_SLOTS.lock().await;
                                    if slots[slot_idx].occupied && slots[slot_idx].addr == Some(addr) {
                                        slots[slot_idx].last_packet_time = Some(Instant::now());
                                    }
                                }
                            }
                        }
                    }
                    _ = notify.notified() => {
                        println!("UDP Listener stop signal received");
                        break;
                    }
                }
            }
        }
    });

    // Auto-disconnect background task
    tokio::spawn({
        let notify = notifylistener.clone();
        let device = Arc::clone(&device);
        async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = Instant::now();
                        let mut to_remove = Vec::new();
                        {
                            let slots = CLIENT_SLOTS.lock().await;
                            for (i, slot) in slots.iter().enumerate() {
                                if slot.occupied {
                                    if let Some(last_time) = slot.last_packet_time {
                                        if now.duration_since(last_time).as_millis() > 3000 {
                                            to_remove.push(i);
                                        }
                                    }
                                }
                            }
                        }
                        for slot_idx in to_remove {
                            println!("Auto-disconnecting inactive client slot {}", slot_idx + 1);
                            release_client_slot(slot_idx + 1, &device).await;
                        }
                    }
                    _ = notify.notified() => {
                        break;
                    }
                }
            }
        }
    });

    "Controller Server started!".to_string()
}


pub async fn stop() -> String {

    cancel_all_clients().await;

    let mut listener_guard = GLOBAL_LISTENER.lock().await;
     if let Some(state) = listener_guard.take() {
        state.notify.notify_waiters();

        println!("Listener stopped signaled");
    } else {
        println!("Listener was not running")
    }

    "Controller Server stopped!".to_string()
}

pub async fn cancel_all_clients() {
    let device = Arc::new(VBusDriver);
    for i in 0..MAX_CLIENTS {
        let occupied = {
            let slots = CLIENT_SLOTS.lock().await;
            slots[i].occupied
        };
        if occupied {
            release_client_slot(i + 1, &device).await;
        }
    }
}

pub async fn acquire_client_slot() -> Option<(usize, usize)> {
    let mut slots = CLIENT_SLOTS.lock().await;

    for (i, slot) in slots.iter_mut().enumerate() {
        if !slot.occupied {
            slot.occupied = true;
            let client_id = i + 1; // use slot number as client id (1-based)
            slot.client_number = Some(client_id);
            slot.last_sequence = 0;
            return Some((i, client_id));
        }
    }

    None
}

pub async fn release_client_slot(client_id: usize, device: &Arc<VBusDriver>) {
    let mut slots = CLIENT_SLOTS.lock().await;
    disconnect_client(client_id - 1).await;
    
    let result: Result<i32, Box<dyn Any + Send + 'static>> = std::panic::catch_unwind(|| {
        device.unplug(client_id as u32) 
    });

    match result {
        Ok(0) => println!("Failed to unplug controller {}", client_id),
        Ok(_) => println!("Successfully unplugged controller {}", client_id),
        Err(e) => println!("Failed to unplug controller {}: panic occurred - {:?}", client_id, e),
    }

    let slot_index = client_id - 1;
    if slot_index < MAX_CLIENTS {
        slots[slot_index].occupied = false;
        slots[slot_index].client_number = None;
        slots[slot_index].last_sequence = 0;
        slots[slot_index].addr = None;
        slots[slot_index].last_packet_time = None;
    }
}

pub async fn disconnect_client(index: usize) {
    let mut clients = CONNECTED_CLIENTS.lock().await;
    if index < clients.len() {
        clients.remove(index);
    }
}

pub async fn add_connected_client(addr: SocketAddr, slot_index: usize) -> Option<ClientInfo> {
    let mut clients = CONNECTED_CLIENTS.lock().await;
    if clients.len() >= 4 {
        return None;
    }

    let ip = addr.ip().to_string();
    let info = ClientInfo { ip, slot: slot_index + 1 };
    clients.push(info.clone());
    Some(info)
}