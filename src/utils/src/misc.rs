use glob::{glob_with, MatchOptions};
use log::{debug, error};
use openssl::sha::Sha1;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Get files by pattern
pub fn get_files_by_pattern(
    pattern: &str,
    case_sensitive: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Create a vector to store the matched files
    let mut files = Vec::new();

    let mut options = MatchOptions::default();
    options.case_sensitive = case_sensitive;

    // Iterate over the matching paths
    for entry in glob_with(pattern, options)? {
        match entry {
            Ok(path) => {
                // Call add_file for each matched file
                if path.is_file() {
                    files.push(path);
                }
            }
            Err(e) => error!(
                "Error matching pattern.\nError: {}\nPattern: {}",
                e, pattern
            ),
        }
    }

    // Return the vector of matched files
    Ok(files)
}

pub fn get_files_by_patterns(
    patterns: Vec<String>,
    case_sensitive: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Create a vector to store the matched files
    let mut files = Vec::new();

    // Iterate over the patterns
    for pattern in patterns {
        debug!("Searching for pattern: {:?}", pattern);
        // Call get_files_by_pattern for each pattern
        let mut pattern_files = get_files_by_pattern(&pattern, case_sensitive)?;
        files.append(&mut pattern_files);
    }

    // Return the vector of matched files
    Ok(files)
}

pub fn file_name_checksum(abs_file_path: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(abs_file_path.as_bytes());
    // make shure the hex encoded is always the same length
    format!("{:0>40}", hex::encode(hasher.finish()))
}

pub fn exit_after_user_input(message: &str, exit_code: i32) -> ! {
    write!(std::io::stdout(), "{}", message).unwrap();
    std::io::stdout().flush().unwrap();
    let _ = std::io::stdin().read(&mut [0u8]).unwrap();
    std::process::exit(exit_code)
}

pub fn wait_for_user_input(message: &str) {
    write!(std::io::stdout(), "{}", message).unwrap();
    std::io::stdout().flush().unwrap();
    let _ = std::io::stdin().read(&mut [0u8]).unwrap();
}
