mod unpacker_tests;
use clap::{Arg, ArgAction, Command};
use config::workflow::Algorithm;
use crypto::{decrypt_evidence, get_file_sha1, get_metadata, load_private_key, EncryptionMeta};
use log::{debug, error, info, warn, LevelFilter};
use logging::Logger;
use report::{ENCRYPTION_PATH, METADATA_PATH, STORAGE_DIR};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    vec,
};
use storage::{read_metadata, FileMeta};
use utils::sanitize::sanitize_dirname;
use zip::ZipArchive;

fn main() {
    let matches = get_command().get_matches();

    let logger = Logger::init()
        .set_level(match matches.get_flag("verbose") {
            true => LevelFilter::Debug,
            false => LevelFilter::Info,
        })
        .apply();

    if let Err(e) = run(matches) {
        error!("{}", e);
        std::process::exit(1);
    }

    logger.finish();
}

fn get_command() -> Command {
    Command::new("Unpacker")
        .version("1.0")
        .about("Unpacks an encrypted archive")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("INPUT")
                .required(true)
                .help("The report directory to decrypt and unpack. It must contain the encryption.json file"),
        )
        .arg(
            Arg::new("private_key")
                .short('k')
                .long("private")
                .value_name("PRIVATE_KEY")
                .help("The private key to decrypt the archive"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("OUTPUT")
                .help("The output directory to unpack the archive"),
        )
        .arg(
            Arg::new("restore")
                .short('r')
                .long("restore")
                .action(ArgAction::SetTrue)
                .help("Restore the stored files with their original names")
        )
        .arg(
            Arg::new("verify")
                .long("verify")
                .action(ArgAction::SetTrue)
                .default_value("true")
                .help("Verify the checksums of the metadata file")
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enables verbose logging")
                .action(clap::ArgAction::SetTrue),
        )
}

pub fn run(matches: clap::ArgMatches) -> Result<(), String> {
    let report_dir: PathBuf = PathBuf::from(matches.get_one::<String>("input").unwrap());
    if !report_dir.exists() {
        return Err(format!(
            "Report directory {:?} does not exist",
            report_dir.display()
        ));
    }

    // Check if the report was archived or not
    let archive_path = Path::new(&report_dir).join(report::ZIP_PATH);
    let storage_dir = Path::new(&report_dir).join(STORAGE_DIR);

    // if both exist or does not exist, it is an error
    if archive_path.exists() == storage_dir.exists() {
        return Err(format!(
            "Expected either {:?} or {:?} to exist, but not both or none",
            archive_path.display(),
            storage_dir.display()
        ));
    }

    let is_archived = archive_path.exists();

    // if is_archived, we expect the "encryption.json" to exist
    let mut encryption_metadata = EncryptionMeta::default();
    if is_archived {
        let meta_path = Path::new(&report_dir).join(ENCRYPTION_PATH);
        if !meta_path.exists() {
            return Err(format!(
                "Metadata file {:?} does not exist",
                meta_path.display()
            ));
        }
        encryption_metadata = get_metadata(&meta_path)
            .map_err(|e| format!("Failed to read metadata file {:?}: {}", ENCRYPTION_PATH, e))?;
    }

    // Determine the output directory
    // - if archived && user supplied an output directory -> use it
    // - if archived && not user supplied -> create new directory inside the report directory
    // - if not archived -> ignore the user supplied output directory
    let output_path: PathBuf = if is_archived {
        let path = match matches.get_one::<String>("output") {
            Some(output) => PathBuf::from(output),
            None => Path::new(&report_dir).join("output"),
        };
        if path.exists() {
            return Err(format!(
                "Output directory {:?} already exists. Please remove it or specify a different directory",
                path.display()
            ));
        }
        path
    } else {
        // warn the user that the output directory will be ignored
        if matches.get_one::<String>("output").is_some() {
            warn!("Output directory will be ignored because the report is not archived");
        }
        report_dir.clone()
    };

    // Edge case: if the archive had been decrypted before but an error occurred
    // we want to avoid decrypting it again
    // So we have to check if the file magic is correct
    let already_decrypted = is_archived
        && encryption_metadata.algorithm != Algorithm::None
        && is_valid_zip_archive(&archive_path);

    if already_decrypted {
        warn!("The archive has already been decrypted: skipping decryption");
    }

    // check if decryption is needed
    if !already_decrypted && is_archived && encryption_metadata.algorithm != Algorithm::None {
        // load private key
        let private_key_file = matches.get_one::<String>("private_key").unwrap();
        if !Path::new(&private_key_file).exists() {
            return Err(format!(
                "Private key file {:?} does not exist",
                private_key_file
            ));
        }
        let private_key = load_private_key(PathBuf::from(&private_key_file)).unwrap();

        // decrypt the evidence
        info!("Decrypting archive");
        decrypt_evidence(Path::new(&archive_path), private_key, encryption_metadata)
            .map_err(|e| format!("Failed to decrypt archive: {}", e))?;

        info!("Decrypted archive");
    }

    // check if extraction is needed
    if is_archived {
        info!("Unpacking archive to {:?}", output_path.display());
        let file = std::fs::File::open(&archive_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        match archive.extract(&output_path) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to extract archive: {}", e);
            }
        }

        info!("Unpacked archive to {:?}", output_path.display());
    }

    // check if user wants to verify the checksums of the metadata file
    let verify = matches.get_flag("verify");
    // check if user wants to extract the files with their original names
    let restore = matches.get_flag("restore");

    // if not any of the above, return
    if !verify && !restore {
        return Ok(());
    }

    // load the metadata file
    let metadata_path = Path::new(&output_path).join(METADATA_PATH);
    if !metadata_path.exists() {
        return Err(format!(
            "Metadata file {:?} does not exist",
            metadata_path.display()
        ));
    }
    let file_metadata = read_metadata(&metadata_path);

    // check if any of the records has a checksum
    let has_checksums = file_metadata
        .iter()
        .any(|record| !record.sha1_checksum.is_empty());

    if verify && !has_checksums {
        warn!("No checksums found in metadata file: skipping verification");
    }

    for record in file_metadata {
        let file_name_checksum = &record.path_checksum;

        // check if we have a valid checksum
        if file_name_checksum.len() != 40 {
            warn!(
                "Invalid checksum found in metadata file: {:?}",
                file_name_checksum
            );
            continue;
        }

        // search for the corresponding file in the output directory
        let file_path = Path::new(&output_path)
            .join(STORAGE_DIR)
            .join(&file_name_checksum);
        if !file_path.exists() {
            error!("File {:?} does not exist", file_path.display());
            continue;
        }

        // verify checksums
        if verify && has_checksums {
            verify_checksum(&file_path, &record)?;
        }

        if restore {
            restore_file(&output_path, &file_path, &record)?;
        }
    }

    Ok(())
}

fn verify_checksum(file_path: &PathBuf, record: &FileMeta) -> Result<bool, String> {
    match get_file_sha1(file_path) {
        Ok(checksum) => {
            if record.sha1_checksum.is_empty() {
                warn!(
                    "Checksum not found for file {:?}: skipping verification",
                    file_path.display()
                );
                return Ok(false);
            }
            if checksum != record.sha1_checksum {
                warn!(
                    "Checksum mismatch for file {:?}: expected {}, got {}",
                    file_path.display(),
                    record.sha1_checksum,
                    checksum
                );
                return Ok(false);
            } else {
                debug!("Checksum verified for file {:?}", file_path.display());
                return Ok(true);
            }
        }
        Err(e) => {
            error!(
                "Failed to calculate checksum for file {:?}: {}",
                file_path.display(),
                e
            );
            return Err(format!(
                "Failed to calculate checksum for file {:?}: {}",
                file_path.display(),
                e
            ));
        }
    }
}

fn path_to_storage_location(file_path: &String, output_path: &Path) -> PathBuf {
    // The path has to be reconstructed inside the storage directory
    // The original path looks like: \\?\C:\Users\user\Documents\file.txt
    // And should be stored like: output\stored_files\C\Users\user\Documents\file.txt

    // Step 1: Strip the "\\?\" prefix from the original path
    let relative_path = file_path.strip_prefix("\\\\?\\").unwrap_or(&file_path);
    // Now looks like:
    // Windows: C:\Users\user\Documents\report\output\storage\file.txt
    // Unix:    /home/user/Documents/report/output/storage/file.txt

    // Step 2: Split the path into components
    let components: Vec<&str> = relative_path.split(|c| c == '\\' || c == '/').collect();
    // Now looks like:
    // Windows: ["C:", "Users", "user", "Documents", "report", "output", "storage", "file.txt"]
    // Unix:    ["", "home", "user", "Documents", "report", "output", "storage", "file.txt"]

    // Edge case: Unix paths start with an empty string, so we have to remove it
    let components: Vec<&str> = if components[0].is_empty() {
        components[1..].to_vec()
    } else {
        components
    };

    // Step 3: Sanitize the path components to be used as directory names
    // Note: sanitize_dirname returns a String, but we need a str
    let components: Vec<String> = components.iter().map(|c| sanitize_dirname(c)).collect();
    // Now looks like:
    // Windows: ["C", "Users", "user", "Documents", "report", "output", "storage", "file.txt"]
    // Unix:    ["home", "user", "Documents", "report", "output", "storage", "file.txt"]

    // Step 4: Create the new path by joining the original path relative to the storage directory
    // Edge case: If output_path already ends with STORAGE_DIR, we have to remove it
    let output_path = if output_path.ends_with(STORAGE_DIR) {
        output_path.parent().unwrap()
    } else {
        output_path
    };

    let separator = if cfg!(windows) { "\\" } else { "/" };
    let new_path = Path::new(&output_path)
        .join(STORAGE_DIR)
        .join(components.join(separator));
    // Now looks like:
    // Windows: output\storage\C\Users\user\Documents\report\output\storage\file.txt
    // Unix:    output/storage/home/user/Documents/report/output/storage/file.txt

    new_path
}

fn restore_file(output_path: &Path, file_path: &Path, record: &FileMeta) -> Result<(), String> {
    let new_path = path_to_storage_location(&record.original_path, output_path);

    // Skip if the file already exists
    if new_path.exists() {
        warn!("File {:?} already exists: skipping", new_path.display());
        return Ok(());
    }

    // Skip if the file is not inside the output directory
    if !new_path.starts_with(output_path) {
        warn!(
            "File {:?} is not inside the output directory: skipping",
            new_path.display()
        );
        return Ok(());
    }

    // We want to preserve the directory structure of the original files
    // so we have to create the directories if they don't exist
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {:?}: {}", parent.display(), e))?;
    }

    // Move the file to the new path
    debug!(
        "Moving file {:?} to {:?}",
        file_path.display(),
        new_path.display()
    );
    fs::rename(&file_path, &new_path).map_err(|e| {
        format!(
            "Failed to move file {:?} to {:?}: {}",
            file_path.display(),
            new_path.display(),
            e
        )
    })
}

fn is_valid_zip_archive(file_path: &Path) -> bool {
    // The first 4 bytes of an encrypted zip archive are always the same
    // 0x50 0x4B 0x03 0x04
    // PK..

    let mut buf = vec![0u8; 4];

    let mut file = match std::fs::File::open(file_path) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open file {:?}: {}", file_path.display(), e);
            return false;
        }
    };

    match file.read_exact(&mut buf) {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to read file {:?}: {}", file_path.display(), e);
            return false;
        }
    }

    buf == [0x50, 0x4B, 0x03, 0x04]
}
