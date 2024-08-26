use log::{error, info};
use std::{env, error::Error, path::Path};
use utils::misc::exit_after_user_input;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(all(unix, not(target_os = "macos")))]
mod unix;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use libc;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn is_elevated() -> bool {
    // Assume not elevated if we don't know how to check
    false
}

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    windows::is_elevated()
}

pub fn run_elevated<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    {
        return windows::run_elevated(path);
    }

    #[cfg(target_os = "macos")]
    {
        return macos::run_elevated(path);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return unix::run_elevated(path);
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform".into())
}

pub fn restart_elevated() {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get current exe: {}", e);
            exit_after_user_input("Press any key to exit...", 1);
        }
    };

    info!("Restarting {:?} as admin", &current_exe.to_string_lossy());
    let res = run_elevated(&current_exe);
    match res {
        Ok(_) => {
            std::process::exit(0);
        }
        Err(e) => {
            error!("Failed to restart as admin: {}", e);
            exit_after_user_input("Press any key to exit...", 1);
        }
    }
}
