use std::{sync::Arc, time::Instant};
use lazy_static::lazy_static;
use serde::Serialize;
use tokio::sync::{Mutex, Notify};
use winapi::{shared::guiddef::GUID, um::{handleapi::INVALID_HANDLE_VALUE, winioctl::{FILE_DEVICE_BUS_EXTENDER, METHOD_BUFFERED}, winnt::{FILE_READ_DATA, FILE_WRITE_DATA}}};

pub const MAX_CLIENTS: usize = 4;
pub const BUFFER_SIZE: usize = 17;
pub const FEEDBACK_BUFFER_LENGTH: usize = 9;

const FILE_DEVICE_BUSENUM: u32 = FILE_DEVICE_BUS_EXTENDER;
const IOCTL_BUSENUM_BASE: u32 = 0x801;

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

pub const IOCTL_BUSENUM_PLUGIN_HARDWARE: u32 = ctl_code(FILE_DEVICE_BUSENUM, IOCTL_BUSENUM_BASE + 0x0, METHOD_BUFFERED, FILE_WRITE_DATA);
pub const IOCTL_BUSENUM_UNPLUG_HARDWARE: u32 = ctl_code(FILE_DEVICE_BUSENUM, IOCTL_BUSENUM_BASE + 0x1, METHOD_BUFFERED, FILE_WRITE_DATA);
pub const IOCTL_BUSENUM_REPORT_HARDWARE: u32 = ctl_code(FILE_DEVICE_BUSENUM, IOCTL_BUSENUM_BASE + 0x3, METHOD_BUFFERED, FILE_WRITE_DATA | FILE_READ_DATA);

pub const GUID_DEVINTERFACE_SCPVBUS: GUID = GUID {
    Data1: 0xf679f562,
    Data2: 0x3164,
    Data3: 0x42ce,
    Data4: [0xa4, 0xdb, 0xe7, 0xdd, 0xbe, 0x72, 0x39, 0x9],
};


#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct XinputGamepad {
    pub w_buttons: u16,
    pub b_left_trigger: u8,
    pub b_right_trigger: u8,
    pub s_thumb_lx: i16,
    pub s_thumb_ly: i16,
    pub s_thumb_rx: i16,
    pub s_thumb_ry: i16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct BusenumUnplugHardware {
    pub size: u32,
    pub serial_no: u32,
    pub flags: u32,
    pub reserved: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ClientSlot {
    pub occupied: bool,
    pub client_number: Option<usize>,
    pub last_sequence: u32,
    pub addr: Option<std::net::SocketAddr>,
    pub last_packet_time: Option<Instant>,
}

// UdpClientInfo removed since data is in ClientSlot

#[derive(Serialize, Clone)]
pub struct ServerInfo {
    pub ip: String,
    pub port: String,
}

#[derive(Clone)]
pub struct ListenerState {
    pub notify: Arc<Notify>,
}

#[derive(Serialize, Clone)]
pub struct ClientInfo {
    pub ip: String,
    pub slot: usize,
}

pub type HANDLE = *mut winapi::ctypes::c_void;
pub const INVALID_HANDLE: HANDLE = INVALID_HANDLE_VALUE;

pub static mut G_V_DEVICE: [bool; MAX_CLIENTS] = [false; MAX_CLIENTS];
pub static mut BUS_HANDLE: HANDLE = INVALID_HANDLE;

lazy_static! {
    pub static ref GLOBAL_LISTENER: Mutex<Option<ListenerState>> = Mutex::new(None);

    pub static ref CLIENT_SLOTS: Mutex<[ClientSlot; MAX_CLIENTS]> = Mutex::new([
        ClientSlot { occupied: false, client_number: None, last_sequence: 0, addr: None, last_packet_time: None },
        ClientSlot { occupied: false, client_number: None, last_sequence: 0, addr: None, last_packet_time: None },
        ClientSlot { occupied: false, client_number: None, last_sequence: 0, addr: None, last_packet_time: None },
        ClientSlot { occupied: false, client_number: None, last_sequence: 0, addr: None, last_packet_time: None },
    ]);

    pub static ref CONNECTED_CLIENTS: Mutex<Vec<ClientInfo>> = Mutex::new(Vec::new());
    pub static ref SERVER_IP: Mutex<ServerInfo> = Mutex::new(ServerInfo { ip: "40405".to_string(), port: "0.0.0.0".to_string() });

}
