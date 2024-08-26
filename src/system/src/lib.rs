use core::panic;
use dirs;
use privileges::is_elevated;
use std::{collections::HashMap, fmt, path::PathBuf};
use whoami;

pub const CUSTOM_FILES_DIR: &str = "custom_files";

#[derive(Debug, Clone)]
pub struct SystemVariables {
    pub os: String,
    pub arch: String,
    pub is_elevated: bool,
    pub distro: String,
    pub base_path: PathBuf,
    pub device_name: String,
    pub user_home: PathBuf,
    pub user: String,
    pub loot_directory: PathBuf,
    pub custom_files_directory: PathBuf,
}

impl SystemVariables {
    pub fn new() -> Self {
        let base_path = get_base_path();
        let custom_files_directory = base_path.join(CUSTOM_FILES_DIR);

        Self {
            os: get_os(),
            arch: get_arch(),
            is_elevated: is_elevated(),
            distro: whoami::distro(),
            base_path: base_path,
            device_name: whoami::devicename(),
            user_home: get_user_home(),
            user: whoami::username(),
            loot_directory: PathBuf::new(),
            custom_files_directory: custom_files_directory,
        }
    }

    #[allow(dead_code)]
    fn loot_directory(&mut self) -> &mut PathBuf {
        &mut self.loot_directory
    }

    pub fn as_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert(
            "BASE_PATH".to_string(),
            self.base_path.to_string_lossy().to_string(),
        );
        map.insert("DEVICE_NAME".to_string(), self.device_name.clone());
        map.insert(
            "USER_HOME".to_string(),
            self.user_home.to_string_lossy().to_string(),
        );
        map.insert("USER_NAME".to_string(), self.user.clone());
        map.insert(
            "LOOT_DIR".to_string(),
            self.loot_directory.to_string_lossy().to_string(),
        );
        map.insert(
            "CUSTOM_FILES_DIR".to_string(),
            self.custom_files_directory.to_string_lossy().to_string(),
        );
        map.insert("OS".to_string(), self.os.clone());
        map.insert("ARCH".to_string(), self.arch.clone());
        map
    }
}

impl fmt::Display for SystemVariables {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut map = self.as_map();
        map.remove("LOOT_DIR");

        write!(f, "System Variables:\n")?;
        for (key, value) in map {
            write!(f, "{}: {}\n", key, value)?;
        }

        Ok(())
    }
}

fn get_user_home() -> PathBuf {
    match dirs::home_dir() {
        Some(path) => path,
        None => PathBuf::new(),
    }
}

// possible bin subdirectories (windows, macos, linux)
const BIN_SUBDIRS: [&str; 3] = ["windows", "macos", "linux"];

/// Returns the base path where this application stores its data
pub fn get_base_path() -> PathBuf {
    // get current exe and retun the parent dir of it
    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            panic!("Error getting current exe: {}", e);
        }
    };

    // remove the filename from the path
    let current_path = match current_exe.parent() {
        Some(path) => path.to_path_buf(),
        None => PathBuf::new(),
    };

    let parent_dir = match current_path.parent() {
        Some(path) => path.to_path_buf(),
        None => PathBuf::new(),
    };

    // if we are inside the bin directory (or its subdirectories), we are in production mode
    // .../bin
    if current_path.file_name().unwrap() == "bin" {
        return parent_dir;
    }
    // if we are inside the bin subdirectories, we are in production mode
    // .../bin/windows
    else if parent_dir.file_name().unwrap() == "bin"
        && BIN_SUBDIRS.contains(&current_path.file_name().unwrap().to_str().unwrap())
    {
        let mut parent_dir = parent_dir.clone();
        // .../bin
        parent_dir.pop();
        // .../
        return parent_dir;
    }
    // check if test
    else if current_path.file_name().unwrap() == "deps" {
        // we fake the base path by returning the output directory in the project root
        let mut parent_dir = parent_dir.clone();
        // .../target/debug
        parent_dir.pop();
        // .../target
        parent_dir.pop();
        // .../
        parent_dir.push("output");
        // .../output
        return parent_dir;
    }
    // we are in debug mode
    // we fake the base path by returning the output directory in the project root
    else if current_path.file_name().unwrap() == "debug" {
        let mut parent_dir = parent_dir.clone();
        // .../target
        parent_dir.pop();
        // .../
        parent_dir.push("output");
        // .../output
        return parent_dir;
    } else {
        // no idea where we are, panic
        panic!("Unknown directory structure. Make sure the application is inside the /bin directory for production");
    }
}

fn get_arch() -> String {
    #[cfg(target_arch = "x86")]
    return "x86".to_string();

    #[cfg(target_arch = "x86_64")]
    return "x86_64".to_string();

    #[cfg(target_arch = "aarch64")]
    return "aarch64".to_string();

    #[cfg(target_arch = "arm")]
    return "arm".to_string();

    #[cfg(target_arch = "mips")]
    return "mips".to_string();

    #[cfg(target_arch = "powerpc")]
    return "powerpc".to_string();

    #[cfg(target_arch = "riscv32")]
    return "riscv32".to_string();

    #[cfg(target_arch = "riscv64")]
    return "riscv64".to_string();

    #[cfg(target_arch = "s390x")]
    return "s390x".to_string();

    #[cfg(target_arch = "sparc")]
    return "sparc".to_string();

    #[cfg(target_arch = "wasm32")]
    return "wasm32".to_string();

    #[cfg(target_arch = "wasm64")]
    return "wasm64".to_string();
}

fn get_os() -> String {
    let machine_kind = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(unix) {
        "linux"
    } else {
        "unknown"
    };

    machine_kind.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_base_path() {
        let base_path = get_base_path();
        assert!(base_path.exists());
    }

    #[test]
    fn test_get_user_home() {
        let user_home = get_user_home();
        assert!(user_home.exists());
    }

    #[test]
    fn test_get_arch() {
        let arch = get_arch();
        assert!(!arch.is_empty());
    }

    #[test]
    fn test_get_os() {
        let os = get_os();
        assert!(!os.is_empty());
    }
}
