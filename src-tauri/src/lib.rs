mod types;
mod vbus;
mod handler;

use std::{any::Any, sync::Arc};

use tauri::tray::TrayIconBuilder;

use crate::{handler::{disconnect_client_by_index, get_connected_clients, get_server, start_server, stop_server}, vbus::driver::{ControllerDevice, VBusDriver}};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;
            let device = Arc::new(VBusDriver);
            for i in 1..5 {
                let result: Result<i32, Box<dyn Any + Send + 'static>> = std::panic::catch_unwind(|| {
                    device.unplug(i) 
                });

                match result {
                    _ => {
                        println!("Cancellation requested for client");
                    }
                }
            }
            let bus = device.is_vbus_exists();
            // if(!bus){
                
            // }
            Ok(())
            })
        .invoke_handler(tauri::generate_handler![start_server, stop_server, get_connected_clients, disconnect_client_by_index, get_server])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
