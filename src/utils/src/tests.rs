use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

// stores all files or directories that are to be deleted after the test
pub struct Cleanup {
    pub paths: Vec<PathBuf>,
}

impl Cleanup {
    pub fn new() -> Cleanup {
        Cleanup { paths: Vec::new() }
    }

    pub fn add(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    pub fn tmp_dir(&mut self, dir_name: &str) -> PathBuf {
        let tmp_dir = std::env::temp_dir().join(dir_name);

        // check if directory already exists
        if tmp_dir.exists() {
            // remove the directory
            panic!("Directory already exists: {:?}", tmp_dir);
        }

        // create the directory
        fs::create_dir(&tmp_dir).unwrap();
        self.add(tmp_dir.clone());

        // Convert to long path on Windows
        #[cfg(windows)]
        {
            if let Some(long_path) = get_long_path_name(&tmp_dir) {
                return long_path;
            }
        }

        tmp_dir
    }

    pub fn create_files(&mut self, dir: &Path, files: Vec<&str>) {
        for file in files {
            let file_path = dir.join(file);
            let _ = std::fs::create_dir_all(file_path.parent().unwrap());
            let mut file = File::create(file_path).unwrap();
            file.write_all(b"").unwrap();
        }
    }
}

#[cfg(windows)]
fn get_long_path_name<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use winapi::shared::minwindef::DWORD;
    use winapi::um::fileapi::GetLongPathNameW;
    use winapi::um::winnt::WCHAR;

    let path = path.as_ref();
    let path_str = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut buffer: Vec<WCHAR> = Vec::with_capacity(261); // MAX_PATH is 260

    unsafe {
        let ret = GetLongPathNameW(
            path_str.as_ptr(),
            buffer.as_mut_ptr(),
            buffer.capacity() as DWORD,
        );

        if ret == 0 || ret > buffer.capacity() as DWORD {
            return None;
        }

        buffer.set_len(ret as usize);
        Some(PathBuf::from(std::ffi::OsString::from_wide(&buffer)))
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            if !path.exists() {
                continue;
            }
            // check if file or directory
            if path.is_file() {
                fs::remove_file(path).unwrap();
            } else {
                fs::remove_dir_all(path).unwrap();
            }
        }
    }
}
