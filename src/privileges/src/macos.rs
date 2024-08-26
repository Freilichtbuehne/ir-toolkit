extern crate core_foundation;
extern crate security_framework_sys;

use security_framework_sys::authorization::*;
use std::error::Error;
use std::ffi::CString;
use std::path::Path;
use std::ptr;

const KAUTHORIZATION_RIGHT_EXECUTE: &str = "system.privilege.admin";

pub fn run_elevated<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn Error>> {
    let path_cstr = CString::new(path.as_ref().to_str().unwrap())?;
    let mut auth_ref: AuthorizationRef = ptr::null_mut();

    unsafe {
        AuthorizationCreate(
            ptr::null(),
            ptr::null(),
            AuthorizationFlags::default(),
            &mut auth_ref,
        );
        let rights = AuthorizationItemSet {
            count: 1,
            items: &mut AuthorizationItem {
                name: KAUTHORIZATION_RIGHT_EXECUTE.as_ptr() as *const i8,
                valueLength: 0,
                value: ptr::null_mut(),
                flags: 0,
            },
        };
        AuthorizationCopyRights(
            auth_ref,
            &rights,
            ptr::null_mut(),
            AuthorizationFlags::default(),
            ptr::null_mut(),
        );

        let result = AuthorizationExecuteWithPrivileges(
            auth_ref,
            path_cstr.as_ptr(),
            AuthorizationFlags::default(),
            ptr::null_mut(),
            ptr::null_mut(),
        );

        AuthorizationFree(auth_ref, AuthorizationFlags::default());

        if result != 0 {
            return Err("Failed to elevate".into());
        }
    }

    Ok(())
}
