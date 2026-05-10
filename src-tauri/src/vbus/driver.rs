pub struct VBusDriver;
use crate::types::{BusenumUnplugHardware, XinputGamepad, BUS_HANDLE, FEEDBACK_BUFFER_LENGTH, GUID_DEVINTERFACE_SCPVBUS, G_V_DEVICE, HANDLE, INVALID_HANDLE, IOCTL_BUSENUM_PLUGIN_HARDWARE, IOCTL_BUSENUM_REPORT_HARDWARE, IOCTL_BUSENUM_UNPLUG_HARDWARE};
use std::{alloc::{alloc, dealloc, Layout}, mem::{self, zeroed}, ptr, slice, thread, time};
use winapi::{ctypes::c_int, shared::{guiddef::GUID, minwindef::{BOOL, DWORD, LPDWORD, LPVOID, MAX_PATH, UCHAR}, ntdef::WCHAR}, um::{errhandlingapi::GetLastError, fileapi::{CreateFileW, OPEN_EXISTING}, ioapiset::DeviceIoControl, minwinbase::LPOVERLAPPED, setupapi::{SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_A, SP_DEVICE_INTERFACE_DETAIL_DATA_W}, winnt::{GENERIC_READ, GENERIC_WRITE}, winuser::{INPUT_u, MapVirtualKeyW, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, LPINPUT}}};

impl ControllerDevice for VBusDriver {
    fn plug_in(&self, user_index: u32) -> BOOL {
        let mut out: BOOL = 0;

    if user_index < 1 || user_index > 4 {
        return out; 
    }

    unsafe {
        if BUS_HANDLE == INVALID_HANDLE {
            BUS_HANDLE = self.get_vxbus_handle();
        }

        if BUS_HANDLE == INVALID_HANDLE {
            return out;
        }
    }

    let transferred: DWORD = 0;
    let mut buffer: [UCHAR; 16] = [0; 16];

    buffer[0] = 0x10; 
    buffer[4] = ((user_index >> 0) & 0xFF) as UCHAR;  
    buffer[5] = ((user_index >> 8) & 0xFF) as UCHAR;  
    buffer[6] = ((user_index >> 16) & 0xFF) as UCHAR; 
    buffer[8] = ((user_index >> 24) & 0xFF) as UCHAR; 

    out = unsafe { DeviceIoControl(
        BUS_HANDLE as HANDLE, 
        IOCTL_BUSENUM_PLUGIN_HARDWARE as DWORD, 
        buffer.as_mut_ptr() as LPVOID, 
        buffer.len() as DWORD, 
        std::ptr::null_mut() as LPVOID, 
        0 as DWORD, 
        transferred as LPDWORD, 
        std::ptr::null_mut() as LPOVERLAPPED, 
    ) };

    if out == 0 {
        let error_code = unsafe { GetLastError() }; 
        println!("DeviceIoControl failed with error code: {}", error_code);
    }

    if out != 0 {
        unsafe { G_V_DEVICE [(user_index - 1) as usize] = true }; 
    }

    out
    }

    fn unplug(&self, user_index: u32) -> BOOL {
        let mut out: BOOL = 0;

        if user_index < 1 || user_index > 4 {
            return out; 
        }

        unsafe {
            if BUS_HANDLE == INVALID_HANDLE {
                BUS_HANDLE = self.get_vxbus_handle();
            }

            if BUS_HANDLE == INVALID_HANDLE {
                return out;
            }
        }
        let transferred: DWORD = 0;
        let mut buffer: BusenumUnplugHardware = unsafe { mem::zeroed::<BusenumUnplugHardware>() };

        buffer.size = size_of::<BusenumUnplugHardware>() as u32;
        buffer.serial_no = user_index;
        buffer.flags = 0x0001;

        out = unsafe { DeviceIoControl(
            BUS_HANDLE as HANDLE, 
            IOCTL_BUSENUM_UNPLUG_HARDWARE as DWORD, 
            &mut buffer as *mut _ as LPVOID, 
            buffer.size as DWORD, 
            std::ptr::null_mut() as LPVOID, 
            0 as DWORD, 
            transferred as LPDWORD, 
            std::ptr::null_mut() as LPOVERLAPPED, 
        ) };

        if out == 0 {
            let error_code = unsafe { GetLastError() }; 
            println!("DeviceIoControl failed with error code: {}", error_code);
        }

        if out != 0 {
            unsafe { G_V_DEVICE [(user_index - 1) as usize] = true; }
        }

        out
    }
    
    fn execute(&self, gamepad: &XinputGamepad, buffer: &mut [u8; 28]) -> bool {
        let mut transferred = 0u32;

        unsafe { 
            
            let gamepad_slice = 
                slice::from_raw_parts(gamepad as *const XinputGamepad as *const u8, mem::size_of::<XinputGamepad>());
            
            buffer[10..(10 + gamepad_slice.len())].copy_from_slice(gamepad_slice);
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

    fn is_vbus_exists(&self) -> bool {
        let mut path: [WCHAR; MAX_PATH] = [0; MAX_PATH];
        
        let n = self.get_vxbus_path(&mut path as *mut WCHAR);

        n > 0
    }

    fn get_vxbus_path(&self, path: *mut WCHAR) -> i32 {
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


    fn get_vxbus_handle(&self) -> HANDLE {
        let mut path: [WCHAR; MAX_PATH] = [0; MAX_PATH];
        
        let n: i32 = self.get_vxbus_path(&mut path as *mut WCHAR);
        if n < 1 {
            return INVALID_HANDLE;
        }

        let path_string = String::from_utf16_lossy(&path);
        println!("Attempting to open device with path: {}", path_string);

        let handle = unsafe { CreateFileW(
            path.as_ptr() as *const u16,
            GENERIC_READ | GENERIC_WRITE,
            0, 
            ptr::null_mut(),  
            OPEN_EXISTING,
            0, 
            ptr::null_mut(),  
        ) };

        if handle == INVALID_HANDLE {
            let error = unsafe { GetLastError() };
            let path_string = String::from_utf16_lossy(&path);
            println!("Failed to open handle for path: {}, error code: {}", path_string, error);
            return INVALID_HANDLE;
        }

        println!("Successfully opened handle: {:?}", handle);

        handle 
    }

        
    fn key_down(&self, fvirtual_key: i32) {  


        let keybd: KEYBDINPUT = unsafe {
            KEYBDINPUT {
                wVk: 0,
                wScan: MapVirtualKeyW(fvirtual_key as u32, 0) as u16,
                dwFlags: 0x0008,
                time: 0,
                dwExtraInfo: 0,
            }
        };
    
        // We need an "empty" winapi struct to union-ize
        let mut input_u: INPUT_u = unsafe { std::mem::zeroed() };
    
        unsafe {
            *input_u.ki_mut() = keybd;
        }
    
        let mut input = INPUT {
            type_: INPUT_KEYBOARD,
            u: input_u,
        };
    
        unsafe { SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int) };
    
        
    }

    /// Release a held key by sending a key-up event and removing it from the held set.
    fn key_up(&self, virtual_key: i32) {

        let keybd: KEYBDINPUT = unsafe {
            KEYBDINPUT {
                wVk: 0,
                wScan: MapVirtualKeyW(virtual_key as u32, 0) as u16,
                dwFlags: 0x0008 | 0x0002,
                time: 0,
                dwExtraInfo: 0,
            }
        };

        // We need an "empty" winapi struct to union-ize
        let mut input_u: INPUT_u = unsafe { std::mem::zeroed() };

        unsafe {
            *input_u.ki_mut() = keybd;
        }

        let mut input = INPUT {
            type_: INPUT_KEYBOARD,
            u: input_u,
        };

        unsafe { SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int) };
    }

}

pub trait ControllerDevice: Send + Sync {
    fn is_vbus_exists(&self) -> bool;
    fn get_vxbus_path(&self, path: *mut WCHAR) -> i32;
    fn get_vxbus_handle(&self) -> HANDLE;
    fn plug_in(&self, user_index: u32) -> BOOL;
    fn unplug(&self, user_index: u32) -> BOOL;
    fn execute(&self, gamepad: &XinputGamepad, buffer: &mut [u8; 28]) -> bool;
    fn key_up(&self, virtual_key: i32);
    fn key_down(&self, virtual_key: i32);
}
