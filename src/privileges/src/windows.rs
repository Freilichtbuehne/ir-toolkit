extern crate winapi;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::{error::Error, path::Path};
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::winuser::SW_SHOWNORMAL;

pub fn run_elevated<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn Error>> {
    let path_wide: Vec<u16> = OsStr::new(path.as_ref())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            "runas\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            path_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if result as i32 <= 32 {
        return Err("Failed to elevate".into());
    }

    Ok(())
}

pub fn is_elevated() -> bool {
    use std::mem;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::winnt::{TokenElevation, HANDLE, TOKEN_ELEVATION, TOKEN_QUERY};

    unsafe {
        let mut token_handle: HANDLE = std::ptr::null_mut();
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length = mem::size_of::<TOKEN_ELEVATION>() as u32;

        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) == 0 {
            return false;
        }

        if GetTokenInformation(
            token_handle,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        ) == 0
        {
            return false;
        }

        elevation.TokenIsElevated != 0
    }
}
