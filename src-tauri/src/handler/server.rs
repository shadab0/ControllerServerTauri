use std::{any::Any, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::{Mutex, Notify}};
use local_ip_address::local_ip;
use tokio_util::sync::CancellationToken;
use crate::{handler::client::ClientHandler, types::{ClientInfo, ListenerState, CLIENT_SLOTS, CLIENT_TOKENS, CONNECTED_CLIENTS, GLOBAL_LISTENER, MAX_CLIENTS, SERVER_IP}, vbus::driver::{ControllerDevice, VBusDriver}};

pub async fn start() -> String {
    let listener = Arc::new(
        TcpListener::bind(format!("{}:40405", local_ip().unwrap()))
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
        listener: Arc::new(Mutex::new(Some(listener.clone()))),
        notify: notifylistener.clone(),
    };

    {
        let mut server_lock = GLOBAL_LISTENER.lock().await;
        *server_lock = Some(listenerstate.clone());
    }

    let device = Arc::new(VBusDriver);

    tokio::spawn({
        let listenerstate = listenerstate.clone();
        let device = Arc::clone(&device);
        async move {
            loop {
                let listener_opt = {
                    let lock = listenerstate.listener.lock().await;
                    lock.as_ref().cloned()
                };

                if let Some(listener) = listener_opt {
                    tokio::select! {
                        res = listener.accept() => {
                            match res {
                                Ok((stream, addr)) => {
                                    if let Some((_, client_id)) = acquire_client_slot().await {
                                        println!("Client {} connected from → slot {}", client_id, client_id);
                                        
                                        let device_inner = Arc::clone(&device);
                                        let mut token_lock = CLIENT_TOKENS[client_id - 1].lock().await;
                                        *token_lock = CancellationToken::new();
                                        let cancel_token = token_lock.clone();
                                        drop(token_lock);

                                        add_connected_client(addr).await;

                                        tokio::spawn(async move {
                                            let mut handler = ClientHandler {
                                                stream,
                                                client_id,
                                                device: device_inner,
                                                cancel_token,
                                            };
                                            handler.init().await;
                                            release_client_slot(client_id).await;
                                            println!("fully released client {}", client_id);
                                        });
                                    } else {
                                        println!("No slots available for connection");
                                    }
                                }
                                Err(e) => {
                                    println!("Error accepting connection: {:?}", e);
                                }
                            }
                        }
                        _ = notifylistener.notified() => {
                            println!("Listener stop signal received");
                        }
                    }
                } else {
                    println!("Listener has been stopped and removed.");
                    break;
                }
            }
        }
    });

    "Controller Server started!".to_string()
}


pub async fn stop() -> String {

    cancel_all_clients_thread().await;

    let mut listener_guard = GLOBAL_LISTENER.lock().await;
     if let Some(state) = listener_guard.take() {
        {
            let mut lock = state.listener.lock().await;
            *lock = None;
        }

        state.notify.notify_one();

        println!("Listener stopped signaled");
    } else {
        println!("Listener was not running")
    }

    let device = Arc::new(VBusDriver);
    for i in 1..5 {

        disconnect_client(i.clone() - 1).await;

        let result: Result<i32, Box<dyn Any + Send + 'static>> = std::panic::catch_unwind(|| {
            device.unplug(i as u32) 
        });

        match result {
            Ok(0) => {
                println!("Failed to unplug controller {}", i);
            }
            Ok(_) => {
                println!("Successfully unplugged controller {}", i);
            }
            Err(e) => {
                println!("Failed to unplug controller {}: panic occurred - {:?}", i, e);
            }
        }
    }


    "Controller Server stopped!".to_string()
}

pub async fn cancel_all_clients_thread() {
    for i in 0..MAX_CLIENTS {
        CLIENT_TOKENS[i].lock().await.cancel();
        disconnect_client(i).await;
        println!("Client {} disconnected", i);
    }
}

pub async fn acquire_client_slot() -> Option<(usize, usize)> {
    let mut slots = CLIENT_SLOTS.lock().await;

    for (i, slot) in slots.iter_mut().enumerate() {
        if !slot.occupied {
            slot.occupied = true;
            let client_id = i + 1; // use slot number as client id (1-based)
            slot.client_number = Some(client_id);
            return Some((i, client_id));
        }
    }

    None
}

pub async fn release_client_slot(slot_index: usize) {
    let mut slots = CLIENT_SLOTS.lock().await;
    CLIENT_TOKENS[slot_index - 1].lock().await.cancel();
    disconnect_client(slot_index - 1).await;
    if slot_index < MAX_CLIENTS {
        slots[slot_index - 1].occupied = false;
        slots[slot_index - 1].client_number = None;
        print!("Slot release for cliennt {:?}", slots);
    }
}

pub async fn disconnect_client(index: usize) {
    let mut clients = CONNECTED_CLIENTS.lock().await;
    if index < clients.len() {
        clients.remove(index);
    }
}

pub async fn add_connected_client(addr: SocketAddr) -> Option<ClientInfo> {
    let mut clients = CONNECTED_CLIENTS.lock().await;
    if clients.len() >= 4 {
        return None;
    }

    let ip = addr.ip().to_string();
    let slot = (1..=4).find(|i| !clients.iter().any(|c| c.slot == *i))?;
    let info = ClientInfo { ip, slot };
    clients.push(info.clone());
    Some(info)
}