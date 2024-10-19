use core::{slice, time};
use std::sync::Mutex;
use rayon::ThreadPoolBuilder;
use tauri::tray::TrayIconBuilder;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::ioapiset::DeviceIoControl;
use winapi::um::minwinbase::LPOVERLAPPED;
use winapi::um::winioctl::{FILE_DEVICE_BUS_EXTENDER, METHOD_BUFFERED};
use winapi::um::winnt::{ FILE_READ_DATA, FILE_WRITE_DATA, GENERIC_READ, GENERIC_WRITE, HANDLE, WCHAR};
use winapi::um::setupapi::*;
use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{BOOL, DWORD, LPDWORD, LPVOID, UCHAR, UINT, ULONG};
use std::mem::{self, zeroed};
use std::alloc::{alloc, dealloc, Layout};
use std::net::{TcpListener, TcpStream};
use std::{ptr, thread};
use std::io::Read;
use std::sync::Arc;
use lazy_static::lazy_static;

const MAX_CLIENTS: usize = 4;
const BUFFER_SIZE: usize = 16;

const MAX_PATH: usize = 260;
pub const AXIS_MAX: i16 = 32767;
pub const AXIS_MIN: i16 = -32768;
pub const FEEDBACK_BUFFER_LENGTH: usize = 9;

// Device IOCTL codes
const FILE_DEVICE_BUSENUM: u32 = FILE_DEVICE_BUS_EXTENDER;
const IOCTL_BUSENUM_BASE: u32 = 0x801;

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

const IOCTL_BUSENUM_PLUGIN_HARDWARE: u32 = ctl_code(FILE_DEVICE_BUSENUM, IOCTL_BUSENUM_BASE + 0x0, METHOD_BUFFERED, FILE_WRITE_DATA);
const IOCTL_BUSENUM_UNPLUG_HARDWARE: u32 = ctl_code(FILE_DEVICE_BUSENUM, IOCTL_BUSENUM_BASE + 0x1, METHOD_BUFFERED, FILE_WRITE_DATA);
const IOCTL_BUSENUM_REPORT_HARDWARE: u32 = ctl_code(FILE_DEVICE_BUSENUM, IOCTL_BUSENUM_BASE + 0x3, METHOD_BUFFERED, FILE_WRITE_DATA | FILE_READ_DATA);
const MAX_NUMBER_XBOX_CTRLS: usize = 4; 
use rayon::ThreadPool;


#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}


#[derive(Clone)]
struct SimulationState {
    running: bool,
}


lazy_static! {
    static ref SIMULATION_STATE: Arc<Mutex<SimulationState>> = Arc::new(Mutex::new(SimulationState {
        running: false,
    }));
}


// // Start the simulation (server, clients, and VBus handling)
// #[tauri::command]
// fn start_simulation(state: Arc<SimulationState>) -> String {
//     let mut running = state.running.lock().unwrap();
//     if *running {
//         return "Simulation is already running!".to_string();
//     }

//     *running = true;
//     let thread_pool = Arc::clone(&state.thread_pool);

//     // Start the simulation in a separate thread
//     thread::spawn(move || {
//         let listener = TcpListener::bind("192.168.0.130:8080").unwrap();
//         println!("Server listening on 192.168.0.130:8080");

//         for (client_id, stream) in listener.incoming().enumerate() {
//             if let Ok(stream) = stream {
//                 let thread_pool = Arc::clone(&thread_pool);
//                 let thread_pool_clone = Arc::clone(&thread_pool);
//                 thread_pool.install(move || {
//                     handle_client(stream, client_id + 1, thread_pool_clone);
//                 });
//             }
//         }
//     });

//     "Simulation started!".to_string()
// }

#[tauri::command]
fn start_simulation() -> String {
    let mut state = SIMULATION_STATE.lock().unwrap();
    if state.running {
        return "Simulation is already running!".to_string();
    }

    state.running = true;
    let listener = TcpListener::bind("192.168.0.130:8080").unwrap();
    let clients = Arc::new(std::sync::Mutex::new(0));
    let thread_pool = Arc::new(ThreadPoolBuilder::new().num_threads(4).build().unwrap());

    println!("Server listening on 192.168.0.130:8080");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        let client_count = {
            let mut count = clients.lock().unwrap();
            if *count >= MAX_CLIENTS {
                println!("Maximum clients reached, rejecting connection.");
                continue;
            }
            *count += 1;
            *count
        };

        let thread_pool = Arc::clone(&thread_pool);

        println!("Client {} connected.", client_count);

        let clients_clone = Arc::clone(&clients);
        thread::spawn(move || {
            handle_client(stream, client_count.clone(), thread_pool);
            unsafe { unplug(client_count.clone() as u32); }
            let mut count = clients_clone.lock().unwrap();
            *count -= 1;
            println!("Client {} disconnected, {} client(s) remaining.", client_count, *count);
        });
    }

    "Simulation started!".to_string()
}

#[tauri::command]
fn stop_simulation() -> String {

    for i in 1..5 {
        let result = std::panic::catch_unwind(|| {
            unsafe { unplug(i) }
        });

        match result {
            Ok(0) => {
                return format!("Failed to unplug controller {}", i);
            }
            Ok(_) => {
                println!("Successfully unplugged controller {}", i);
            }
            Err(e) => {
                return format!("Failed to unplug controller {}: panic occurred - {:?}", i, e);
            }
        }
    }

    let mut state = SIMULATION_STATE.lock().unwrap();
    if !state.running {
        return "Simulation is not running!".to_string();
    }

    state.running = false;



    // Logic to stop the simulation if needed
    "Simulation stopped!".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    for i in 1..5 {
        let result = std::panic::catch_unwind(|| {
            unsafe { unplug(i) }
        });

        // match result {
        //     Ok(0) => {
        //         return format!("Failed to unplug controller {}", i);
        //     }
        //     Ok(_) => {
        //         println!("Successfully unplugged controller {}", i);
        //     }
        //     Err(e) => {
        //         return format!("Failed to unplug controller {}: panic occurred - {:?}", i, e);
        //     }
        // }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;
            Ok(())
            })
        .invoke_handler(tauri::generate_handler![greet, start_simulation, stop_simulation])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


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
    pub size: ULONG,   
    pub serial_no: ULONG,
    pub flags: ULONG, 
    pub reserved: ULONG
}

pub static G_GAMEPAD: Mutex<[XinputGamepad; MAX_NUMBER_XBOX_CTRLS]> = Mutex::new([XinputGamepad {
    w_buttons: 0,
    b_left_trigger: 0,
    b_right_trigger: 0,
    s_thumb_lx: 0,
    s_thumb_ly: 0,
    s_thumb_rx: 0,
    s_thumb_ry: 0,
}; MAX_NUMBER_XBOX_CTRLS]);


// Vector to track if devices are connected
pub static mut G_V_DEVICE: [bool; MAX_NUMBER_XBOX_CTRLS] = [false; MAX_NUMBER_XBOX_CTRLS];
pub static mut BUS_HANDLE: HANDLE = INVALID_HANDLE_VALUE;
pub static mut GAMEPAD_DATA: Mutex<XinputGamepad> = Mutex::new(XinputGamepad {
    w_buttons: 0,
    b_left_trigger: 0,
    b_right_trigger: 0,
    s_thumb_lx: 0,
    s_thumb_ly: 0,
    s_thumb_rx: 0,
    s_thumb_ry: 0,
});


pub fn is_vbus_exists() -> bool {
    let mut path: [WCHAR; MAX_PATH] = [0; MAX_PATH];
    
    let n = get_vxbus_path(&mut path as *mut WCHAR);

    n > 0
}

const GUID_DEVINTERFACE_SCPVBUS: GUID = GUID {
    Data1: 0xf679f562,
    Data2: 0x3164,
    Data3: 0x42ce,
    Data4: [0xa4, 0xdb, 0xe7, 0xdd, 0xbe, 0x72, 0x39, 0x9],
};

fn get_vxbus_path(path: *mut WCHAR) -> i32 {
    unsafe {
        let device_class_guid: GUID = GUID_DEVINTERFACE_SCPVBUS; 
        let mut device_interface_data: SP_DEVICE_INTERFACE_DATA = zeroed();
        device_interface_data.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as DWORD;

        let device_info_set = SetupDiGetClassDevsW(
            &device_class_guid,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );

        if device_info_set.is_null() {
            eprintln!("Failed to get device info set");
            return -1;
        }

        let mut member_index: DWORD = 0;
        let mut found_device = false;

        while SetupDiEnumDeviceInterfaces(
            device_info_set,
            ptr::null_mut(),
            &device_class_guid,
            member_index,
            &mut device_interface_data,
        ) != 0
        {
            found_device = true;

            let mut required_size: DWORD = 0;
            SetupDiGetDeviceInterfaceDetailW(
                device_info_set,
                &mut device_interface_data,
                ptr::null_mut(),
                0,
                &mut required_size,
                ptr::null_mut(),
            );

            let detail_data_layout = Layout::from_size_align(
                required_size as usize,
                std::mem::align_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>(),
            )
            .unwrap();
            let detail_data_buffer = alloc(detail_data_layout) as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;

            if detail_data_buffer.is_null() {
                eprintln!("Failed to allocate memory for detail data buffer");
                SetupDiDestroyDeviceInfoList(device_info_set);
                return -1;
            }

            (*detail_data_buffer).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_A>() as DWORD;

            if SetupDiGetDeviceInterfaceDetailW(
                device_info_set,
                &mut device_interface_data,
                detail_data_buffer,
                required_size,
                &mut required_size,
                ptr::null_mut(),
            ) == 0
            {
                eprintln!("Failed to get device interface detail");
                dealloc(detail_data_buffer as *mut u8, detail_data_layout);
                SetupDiDestroyDeviceInfoList(device_info_set);
                return -1;
            }

            let device_path = (*detail_data_buffer).DevicePath.as_ptr();
            let num_chars = (required_size / size_of::<WCHAR>() as u32) as usize;

            println!("Found device path: {:?}", device_path); 

            for i in 0..num_chars {
                *path.add(i) = *device_path.add(i);
            }

            dealloc(detail_data_buffer as *mut u8, detail_data_layout);
            member_index += 1; 
        }

        if !found_device {
            eprintln!("No devices found for the specified GUID");
        }

        SetupDiDestroyDeviceInfoList(device_info_set);
        return if found_device { member_index as i32 } else { -1 };
    }
}


pub unsafe fn get_vxbus_handle() -> HANDLE {
    let mut path: [WCHAR; MAX_PATH] = [0; MAX_PATH];
    
    let n: i32 = get_vxbus_path(&mut path as *mut WCHAR);
    if n < 1 {
        return INVALID_HANDLE_VALUE;
    }

    let path_string = String::from_utf16_lossy(&path);
    println!("Attempting to open device with path: {}", path_string);

    let handle = CreateFileW(
        path.as_ptr() as *const u16,
        GENERIC_READ | GENERIC_WRITE,
        0, 
        ptr::null_mut(),  
        OPEN_EXISTING,
        0, 
        ptr::null_mut(),  
    );

    if handle == INVALID_HANDLE_VALUE {
        let error = unsafe { GetLastError() };
        let path_string = String::from_utf16_lossy(&path);
        println!("Failed to open handle for path: {}, error code: {}", path_string, error);
        return INVALID_HANDLE_VALUE;
    }

    println!("Successfully opened handle: {:?}", handle);

    handle 
}

pub unsafe fn plug_in(user_index: UINT) -> BOOL {
    let mut out: BOOL = 0;

    if user_index < 1 || user_index > 4 {
        return out; 
    }

    if BUS_HANDLE == INVALID_HANDLE_VALUE {
        BUS_HANDLE = get_vxbus_handle();
    }

    if BUS_HANDLE == INVALID_HANDLE_VALUE {
        return out;
    }

    let transferred: DWORD = 0;
    let mut buffer: [UCHAR; 16] = [0; 16];

    buffer[0] = 0x10; 
    buffer[4] = ((user_index >> 0) & 0xFF) as UCHAR;  
    buffer[5] = ((user_index >> 8) & 0xFF) as UCHAR;  
    buffer[6] = ((user_index >> 16) & 0xFF) as UCHAR; 
    buffer[8] = ((user_index >> 24) & 0xFF) as UCHAR; 

    out = DeviceIoControl(
        BUS_HANDLE as HANDLE, 
        IOCTL_BUSENUM_PLUGIN_HARDWARE as DWORD, 
        buffer.as_mut_ptr() as LPVOID, 
        buffer.len() as DWORD, 
        std::ptr::null_mut() as LPVOID, 
        0 as DWORD, 
        transferred as LPDWORD, 
        std::ptr::null_mut() as LPOVERLAPPED, 
    );

    if out == 0 {
        let error_code = GetLastError(); 
        println!("DeviceIoControl failed with error code: {}", error_code);
    }

    if out != 0 {
        G_V_DEVICE[(user_index - 1) as usize] = true; 
    }

    out
}


pub unsafe fn unplug(user_index: UINT) -> BOOL {
    let mut out: BOOL = 0;

    if user_index < 1 || user_index > 4 {
        return out; 
    }

    if BUS_HANDLE == INVALID_HANDLE_VALUE {
        BUS_HANDLE = get_vxbus_handle();
    }

    if BUS_HANDLE == INVALID_HANDLE_VALUE {
        return out;
    }

    let transferred: DWORD = 0;
    let mut buffer: BusenumUnplugHardware = mem::zeroed::<BusenumUnplugHardware>();

    buffer.size = size_of::<BusenumUnplugHardware>() as u32;
    buffer.serial_no = user_index;
    buffer.flags = 0x0001;

    out = DeviceIoControl(
        BUS_HANDLE as HANDLE, 
        IOCTL_BUSENUM_UNPLUG_HARDWARE as DWORD, 
        &mut buffer as *mut _ as LPVOID, 
        buffer.size as DWORD, 
        std::ptr::null_mut() as LPVOID, 
        0 as DWORD, 
        transferred as LPDWORD, 
        std::ptr::null_mut() as LPOVERLAPPED, 
    );

    if out == 0 {
        let error_code = GetLastError(); 
        println!("DeviceIoControl failed with error code: {}", error_code);
    }

    if out != 0 {
        G_V_DEVICE[(user_index - 1) as usize] = true; 
    }

    out
}


fn handle_client(mut stream: TcpStream, client_id: usize, thread_pool: Arc<rayon::ThreadPool>) {
    let mut buffer: [u8; 16] = [0; BUFFER_SIZE];
    let mut device_buffer: [u8; 28] = [0; 28];
    let mut gamepad = unsafe { mem::zeroed::<XinputGamepad>() };
    let user_index = client_id - 1;

    device_buffer[0] = 0x1C;
    device_buffer[4] = ((client_id >> 0) & 0xFF) as u8;
    device_buffer[5] = ((client_id >> 8) & 0xFF) as u8;
    device_buffer[6] = ((client_id >> 16) & 0xFF) as u8;
    device_buffer[7] = ((client_id >> 24) & 0xFF) as u8;
    device_buffer[9] = 0x14;

    unsafe { print!("{}",plug_in(client_id as u32)); }

    loop {
        match stream.read(&mut buffer) {
            Ok(size) => {
                if size == 0 {
                    println!("Client {} disconnected.", client_id);
                    break;
                }

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

                {
                    let mut gamepad_data =  G_GAMEPAD.lock().unwrap(); 
                    if buffer[14] == 0
                    {
                        gamepad_data[user_index].w_buttons &= if buffer[13] != 1 { !gamepad.w_buttons } else { 0xFFF0 };
                        gamepad_data[user_index].w_buttons |= if buffer[13] != 1 { gamepad.w_buttons * buffer[12] as u16 } else { gamepad.w_buttons };
                        gamepad_data[user_index].b_left_trigger =  gamepad.b_left_trigger;
                        gamepad_data[user_index].b_right_trigger =  gamepad.b_right_trigger;
                        x_output_set_state(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                    }
                    else if buffer[14] == 1 {
                        gamepad_data[user_index].s_thumb_lx = gamepad.s_thumb_lx;
                        gamepad_data[user_index].s_thumb_ly = gamepad.s_thumb_ly;
                       
                        thread_pool.install(|| {
                            x_output_set_state(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                        });
                    }
                    else {
                        gamepad_data[user_index].s_thumb_rx = gamepad.s_thumb_rx;
                        gamepad_data[user_index].s_thumb_ry = gamepad.s_thumb_ry;  

                        thread_pool.install(|| {
                            x_output_set_state(&gamepad_data[user_index].clone(), &mut device_buffer.clone());
                        });
                    }
                }
              
            
            }
            Err(e) => {
                println!("Error with client {}: {:?}", client_id, e);
                break;
            }
        }
    }
}

pub fn x_output_set_state(p_gamepad: &XinputGamepad, buffer: &mut [u8; 28], ) -> bool {

    let mut transferred = 0u32;

    unsafe { 
          
        let gamepad_slice = 
            slice::from_raw_parts(p_gamepad as *const XinputGamepad as *const u8, mem::size_of::<XinputGamepad>());
        
        buffer[10..(10 + gamepad_slice.len())].copy_from_slice(gamepad_slice);
        
        thread::sleep(time::Duration::from_millis(2));
    }

    let mut output: [u8; FEEDBACK_BUFFER_LENGTH] = [0; FEEDBACK_BUFFER_LENGTH];

    let success = unsafe {
        DeviceIoControl(
            BUS_HANDLE,
            IOCTL_BUSENUM_REPORT_HARDWARE,
            buffer.as_ptr() as LPVOID,
            buffer.len() as u32,
            output.as_mut_ptr() as LPVOID,
            output.len() as u32,
            &mut transferred,
            ptr::null_mut(),
        )
    };

    if success == 0 {
        return false;
    }

    true
}

