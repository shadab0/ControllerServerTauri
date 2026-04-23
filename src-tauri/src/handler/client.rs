use std::{mem, ptr, sync::Arc};
use tokio_util::sync::CancellationToken;
use tokio::{io::AsyncReadExt, net::TcpStream};
use crate::{types::{XinputGamepad, BUFFER_SIZE, G_GAMEPAD}, vbus::driver::ControllerDevice};

pub struct ClientHandler<T: ControllerDevice> {
    pub stream: TcpStream,
    pub client_id: usize,
    pub device: Arc<T>,
    pub cancel_token: CancellationToken,
}

impl<T: ControllerDevice + 'static> ClientHandler<T> {
    pub async fn init(&mut self) {
        let mut buffer: [u8; 17] = [0; BUFFER_SIZE];
        let mut device_buffer: [u8; 28] = [0; 28];
        let mut gamepad = unsafe { mem::zeroed::<XinputGamepad>() };
        let user_index = self.client_id - 1;

        device_buffer[0] = 0x1C;
        device_buffer[4] = ((self.client_id >> 0) & 0xFF) as u8;
        device_buffer[5] = ((self.client_id >> 8) & 0xFF) as u8;
        device_buffer[6] = ((self.client_id >> 16) & 0xFF) as u8;
        device_buffer[7] = ((self.client_id >> 24) & 0xFF) as u8;
        device_buffer[9] = 0x14;

        print!("{}", self.device.plug_in(self.client_id as u32)); 

        //let mut runfire = true;

        loop {
            tokio::select! {
                result = self.stream.read_exact(&mut buffer) => {
                    match result {
                        Ok(0) => {
                            println!("Client {} disconnected (EOF).", self.client_id);
                            break;
                        }
                        Err(e) => {
                            eprintln!("Error reading from client {}: {}", self.client_id, e);
                            break;
                        }
                        Ok(_) => {
                            unsafe {
                                ptr::copy_nonoverlapping(
                                    buffer.as_ptr(),              
                                    &mut gamepad as *mut _ as *mut u8, 
                                    12 
                                );
                            }       

                            if buffer[15] > 4 {
                                continue;
                            }

                            if buffer[16] == 1 {
                                if buffer[12] == 1 {
                                    self.device.key_down(gamepad.w_buttons.into());
                                    continue;
                                }
                                
                                self.device.key_up(gamepad.w_buttons.into());
                            }

                            {
                                let mut gamepad_data =  G_GAMEPAD.lock().await; 
                                if buffer[14] == 0
                                {
                                    // if gamepad.w_buttons == 0x4000 {
                                    //     gamepad_data[user_index].w_buttons &= !(0x4000);
                                    //     gamepad_data[user_index].w_buttons |= (0x4000) * 0 as u16;
                                    //     gamepad_data[user_index].b_left_trigger =  gamepad.b_left_trigger;
                                    //     gamepad_data[user_index].b_right_trigger =  gamepad.b_right_trigger;
                                    //     x_output_set_state(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                                    //     gamepad_data[user_index].w_buttons &= !(0x4000);
                                    //     gamepad_data[user_index].w_buttons |= (0x4000) * 1 as u16;
                                    //     gamepad_data[user_index].b_left_trigger =  gamepad.b_left_trigger;
                                    //     gamepad_data[user_index].b_right_trigger =  gamepad.b_right_trigger;
                                    //     x_output_set_state(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                                    //     continue;
                                    // }

                                    gamepad_data[user_index].w_buttons &= if buffer[13] != 1 { !gamepad.w_buttons } else { 0xFFF0 };
                                    gamepad_data[user_index].w_buttons |= if buffer[13] != 1 { gamepad.w_buttons * buffer[12] as u16 } else { gamepad.w_buttons };
                                    gamepad_data[user_index].b_left_trigger = if buffer[13] == 2 { gamepad.b_left_trigger } else { gamepad_data[user_index].b_left_trigger };
                                    gamepad_data[user_index].b_right_trigger = if buffer[13] == 3 { gamepad.b_right_trigger } else { gamepad_data[user_index].b_right_trigger };
                                    self.device.execute(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                                
                                } 
                                else 
                                {
                                    if buffer[14] == 1 {
                                        gamepad_data[user_index].s_thumb_lx = gamepad.s_thumb_lx;
                                        gamepad_data[user_index].s_thumb_ly = gamepad.s_thumb_ly;
                                    } else {
                                        gamepad_data[user_index].s_thumb_rx = gamepad.s_thumb_rx;
                                        gamepad_data[user_index].s_thumb_ry = gamepad.s_thumb_ry;
                                    }

                                    // let gamepad_clone = gamepad_data[user_index].clone();
                                    // let device = Arc::clone(&self.device);
                                    // let mut dev_buf = device_buffer.clone();
                                    //let permit = EXEC_SEMAPHORE.clone().acquire_owned().await.unwrap();

                                    // tokio::spawn(async move {
                                    //     device.execute(&gamepad_clone, &mut dev_buf);
                                    //     //drop(permit);
                                    // });
                                    self.device.execute(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                                }
                            }
                        }         
                    }
                }

                _ = self.cancel_token.cancelled() => {
                    break;
                }
            }
        }
        self.device.unplug(self.client_id as u32);
        println!("Unplug/Cancellation requested for client {}", self.client_id);
    }
}