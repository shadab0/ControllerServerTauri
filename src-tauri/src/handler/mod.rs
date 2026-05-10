use crate::types::{ClientInfo, ServerInfo, CONNECTED_CLIENTS, SERVER_IP};

mod server;

#[tauri::command]
pub async fn start_server() -> String {
    server::start().await
}

#[tauri::command]
pub async fn stop_server() -> String {
    server::stop().await
}

#[tauri::command]
pub async fn get_connected_clients() -> Vec<ClientInfo> {
    CONNECTED_CLIENTS.lock().await.clone()
}

#[tauri::command]
pub async fn get_server() -> ServerInfo {
    SERVER_IP.lock().await.clone()
}

#[tauri::command]
pub async fn disconnect_client_by_index(index: usize) {
    let device = std::sync::Arc::new(crate::vbus::driver::VBusDriver);
    server::release_client_slot(index, &device).await;
}

