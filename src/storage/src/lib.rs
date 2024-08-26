use chrono::{Local, TimeZone};
use chrono_tz::{self, Tz};
use config::workflow::Reporting;
use crypto::{copy_file_with_sha1, encrypt_evidence, EncryptionMeta};
use filetime::FileTime;
use log::{debug, error, info, warn};
use openssl::pkey::Public;
use openssl::rsa::Rsa;
use openssl::sha::Sha1;
use report::{Report, ACTION_LOG_DIR, LOOT_DIR, STORAGE_DIR};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use utils::misc::{file_name_checksum, get_files_by_patterns};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

#[derive(Serialize, Deserialize)]
pub struct FileMeta {
    pub original_path: String,
    pub modified_time: String,
    pub accessed_time: String,
    pub created_time: String,
    pub sha1_checksum: String,
    pub path_checksum: String,
    pub size: u64,
    pub comment: Option<String>,
}

#[derive(Debug)]
pub struct FileProcessor<'a> {
    public_key: Option<Rsa<Public>>,
    zip_writer: Option<ZipWriter<BufWriter<File>>>,
    csv_writer: Option<csv::Writer<BufWriter<File>>>,
    report_settings: Reporting,
    report: &'a Report,
    added_files: HashMap<String, bool>,
}

impl<'a> FileProcessor<'a> {
    pub fn new(report: &'a Report) -> Result<Self, Box<dyn Error>> {
        // initialize csv writer
        let metadata_path = report.metadata_path.clone();
        let metadata_file = match File::create(&metadata_path) {
            Ok(file) => file,
            Err(_) => {
                error!("Failed to create metadata file: {:?}", &metadata_path);
                return Err("Failed to create metadata file".into());
            }
        };
        let metadata_file = BufWriter::new(metadata_file);
        let csv_writer = {
            let writer = csv::Writer::from_writer(metadata_file);
            Some(writer)
        };

        Ok(Self {
            public_key: None,
            zip_writer: None,
            csv_writer: csv_writer,
            report_settings: Reporting::default(),
            report: report,
            added_files: HashMap::new(),
        })
    }

    fn initialize_zip_archive(&mut self) {
        let zip_path = self.report.zip_path.clone();

        let zip_file = match File::create(&zip_path) {
            Ok(file) => file,
            Err(_) => {
                error!("Failed to create zip archive: {:?}", &zip_path);
                return;
            }
        };
        let mut zip_writer = ZipWriter::new(BufWriter::new(zip_file));

        // create directory in the zip archive
        let file_options = SimpleFileOptions::default();
        zip_writer.add_directory(LOOT_DIR, file_options).unwrap();
        let file_options = SimpleFileOptions::default().large_file(true);
        zip_writer.add_directory(STORAGE_DIR, file_options).unwrap();
        let file_options = SimpleFileOptions::default();
        zip_writer
            .add_directory(ACTION_LOG_DIR, file_options)
            .unwrap();

        self.zip_writer = Some(zip_writer);
    }

    pub fn set_public_key(&mut self, public_key: Rsa<Public>) -> &mut Self {
        // warn if the public key is set and encryption is disabled
        if !self.report_settings.zip_archive.encryption.enabled {
            warn!("Setting public key won't have any effect: encryption is disabled");
        }

        self.public_key = Some(public_key);
        self
    }

    pub fn set_report_settings(&mut self, report_settings: Reporting) -> &mut Self {
        self.report_settings = report_settings;

        // check if archiving is enabled
        if self.report_settings.zip_archive.enabled {
            self.initialize_zip_archive();
        }

        self
    }

    pub fn store(
        &mut self,
        file_path: &Path,
        comment: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Step 1: Check if the file exists
        if !file_path.exists() {
            error!("File not found: {:?}", file_path);
            return Err("File not found".into());
        }

        // Step 2: Get the absolute path
        let abs_file_path = match file_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                error!("Failed to get absolute path: {:?}", file_path);
                file_path.to_path_buf()
            }
        };

        debug!("Storing file: {:?}", abs_file_path);

        // Step 3: Initialize metadata
        let mut metadata = FileMeta {
            original_path: abs_file_path.to_str().unwrap().to_string(),
            modified_time: "".to_string(),
            accessed_time: "".to_string(),
            created_time: "".to_string(),
            sha1_checksum: "".to_string(),
            path_checksum: file_name_checksum(&abs_file_path.to_str().unwrap()),
            size: 0,
            comment: comment,
        };

        // Step 4: Get MAC (Modified, Accessed, Created) times
        // check if file is in loot directory
        // if so, we don't need to store the MAC times as they are generated by this framework
        let loot_dir = &self.report.loot_dir;
        let in_loot_dir = abs_file_path.starts_with(loot_dir);
        if self.report_settings.metadata.mac_times && !in_loot_dir {
            debug!("Obtaining MAC times for file");
            let file_metadata = fs::metadata(file_path).unwrap();
            let size = file_metadata.len();

            let mtime = FileTime::from_last_modification_time(&file_metadata);
            let atime = FileTime::from_last_access_time(&file_metadata);
            let ctime = FileTime::from_creation_time(&file_metadata);

            // convert to rfc3339 string
            let tz = Tz::UTC;
            let mtime: String = Local
                .timestamp_opt(mtime.unix_seconds(), 0)
                .unwrap()
                .with_timezone(&tz)
                .to_rfc3339();
            let atime: String = Local
                .timestamp_opt(atime.unix_seconds(), 0)
                .unwrap()
                .with_timezone(&tz)
                .to_rfc3339();
            let ctime: String = match ctime {
                Some(ctime) => Local
                    .timestamp_opt(ctime.unix_seconds(), 0)
                    .unwrap()
                    .with_timezone(&tz)
                    .to_rfc3339(),
                None => "None".to_string(),
            };

            metadata.modified_time = mtime;
            metadata.accessed_time = atime;
            metadata.created_time = ctime;
            metadata.size = size;
        }

        // Step 5: Add file to the archive
        // use the SHA1 checksum of the abs_file_path to avoid duplicate file names
        // enable_archive && loot -> loot_files/[filename]
        // enable_archive && !loot -> STORAGE_DIR/[checksum]
        // !enable_archive && loot -> loot_files/[filename]
        // !enable_archive && !loot -> STORAGE_DIR/[checksum]
        let archive_filename = match in_loot_dir {
            true => {
                // return LOOT_DIR/[filename]
                let file_name = abs_file_path.file_name().unwrap().to_str().unwrap();
                format!("{}/{}", LOOT_DIR, file_name)
            }
            false => {
                // return STORAGE_DIR/[checksum]
                // check if the file was already added to the archive
                // we only check here, as we are dealing with absolute paths
                if self.added_files.contains_key(&metadata.path_checksum) {
                    return Err("File already added to the archive".into());
                }
                format!("{}/{}", STORAGE_DIR, &metadata.path_checksum)
            }
        };

        // Step 6: Add file to the archive
        let enable_archive = self.report_settings.zip_archive.enabled;
        // If archiving is enabled, add the file to the zip archive
        if enable_archive {
            match self.add_file_to_zip(&abs_file_path, archive_filename) {
                Ok(checksum) => metadata.sha1_checksum = checksum,
                Err(e) => {
                    return Err(format!("Failed to add file to zip archive: {:?}", e).into());
                }
            }
        }
        // If archiving is disabled, but checksum enabled, copy the file to the loot directory
        else if self.report_settings.metadata.checksums {
            let loot_file_path = self.report.dir.join(&archive_filename);
            match copy_file_with_sha1(&abs_file_path, &loot_file_path) {
                Ok(checksum) => metadata.sha1_checksum = checksum,
                Err(e) => {
                    return Err(format!(
                        "Failed to copy file from {:?} to {:?}: {:?}",
                        abs_file_path, loot_file_path, e
                    )
                    .into());
                }
            }
        }
        // If archiving and checksum is disabled, copy the file to the loot directory
        else {
            let loot_file_path = self.report.dir.join(&archive_filename);
            match fs::copy(&file_path, &loot_file_path) {
                Ok(_) => (),
                Err(e) => {
                    return Err(format!(
                        "Failed to copy file from {:?} to {:?}: {:?}",
                        file_path, loot_file_path, e
                    )
                    .into());
                }
            }
        }

        // Step 7: Add the file to the added_files hashmap
        if !in_loot_dir {
            self.added_files
                .insert(metadata.path_checksum.clone(), true);
        }

        // Step 8: Write metadata
        if let Some(csv_writer) = &mut self.csv_writer {
            csv_writer.serialize(metadata)?;
            csv_writer.flush()?;
        }

        Ok(())
    }

    /// Adds a single file to the archive by its path
    fn add_file_to_zip(
        &mut self,
        abs_file_path: &PathBuf,
        zip_file_name: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Step 0: Error if the archive is disabled or not initialized
        if self.zip_writer.is_none() {
            return Err("Zip archive is not initialized".into());
        } else if !self.report_settings.zip_archive.enabled {
            return Err("Cannot add file to zip archive: archiving is disabled".into());
        }

        // Step 1: Determine compression method
        let file_size = match fs::metadata(abs_file_path) {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                error!("Failed to get file size of {:?}: {:?}", abs_file_path, e);
                return Err("Failed to get file size".into());
            }
        };

        // Step 2: Set compression options
        let settings = &self.report_settings.zip_archive.compression;
        let method =
            if settings.enabled && (file_size <= settings.size_limit || settings.size_limit == 0) {
                CompressionMethod::ZSTD
            } else {
                CompressionMethod::Stored
            };

        // Check if file is larger than 4 GB
        // See: https://docs.rs/zip/2.1.3/zip/write/struct.FileOptions.html#method.large_file
        // See: https://github.com/zip-rs/zip2/issues/195
        //TODO: invalid crc checksums when unpacking with files larger than 4 GB
        let large_file = file_size > u32::MAX as u64;
        if large_file {
            warn!("Adding files larger than 4 GB to the zip archive");
        }

        let options = SimpleFileOptions::default()
            .large_file(large_file)
            .compression_method(method);

        // Step 3: Open the file
        let file = match File::open(abs_file_path) {
            Ok(file) => file,
            Err(_) => {
                error!("Failed to open file: {:?}", abs_file_path);
                return Err("Failed to open file".into());
            }
        };

        debug!(
            "Adding file {:?} to zip archive: {:?}",
            abs_file_path.display(),
            zip_file_name
        );

        // Step 4: Write the file to the archive
        // Combine this step with checksum calculation to avoid redundant file reads
        let enable_checksum = self.report_settings.metadata.checksums;
        if let Some(writer) = &mut self.zip_writer {
            writer.start_file(zip_file_name, options)?;

            let mut hasher = Sha1::new();
            let mut reader = BufReader::new(file);
            let mut buffer = [0u8; 4096];
            loop {
                let bytes_read = reader.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                if enable_checksum {
                    hasher.update(&buffer[..bytes_read]);
                }
                writer.write_all(&buffer[..bytes_read])?;
            }

            // delete the file if it is inside the report directory
            if abs_file_path.starts_with(&self.report.dir) {
                match fs::remove_file(abs_file_path) {
                    Ok(_) => (),
                    Err(e) => error!("Failed to remove file: {:?}", e),
                }
            }

            match enable_checksum {
                true => {
                    let checksum = hasher.finish();
                    // ensure the checksum has the same length
                    let checksum: String = format!("{:0>40}", hex::encode(checksum));
                    return Ok(checksum);
                }
                false => {
                    return Ok("".to_string());
                }
            }
        }
        Err("Failed to add file to zip archive".into())
    }

    fn write_encryption_metadata(
        &mut self,
        meta: &EncryptionMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encryption_file = File::create(&self.report.encryption_path)?;
        match serde_json::to_writer_pretty(encryption_file, meta) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write encryption metadata: {:?}", e).into()),
        }
    }

    pub fn finish(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let loot_dir = self.report.loot_dir.clone();
        let action_log_dir: PathBuf = self.report.action_log_dir.clone();
        let metadata_path = self.report.metadata_path.clone();
        if !metadata_path.exists() {
            warn!("Metadata file not found: {:?}", metadata_path);
        }

        // if archiving is disabled, we can skip the zip archive creation and encryption
        let archive_enabled = self.report_settings.zip_archive.enabled;
        if !archive_enabled {
            return Ok(());
        }

        info!("Adding all remaining files to the archive");
        let include_files = match get_files_by_patterns(
            vec![
                format!("{}/{}", loot_dir.to_str().unwrap(), "**/*"),
                //format!("{}/{}", loot_dir.to_str().unwrap(), "*"),
                format!("{}/{}", action_log_dir.to_str().unwrap(), "*"),
                format!("{}", metadata_path.to_str().unwrap()),
            ],
            true,
        ) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get files by pattern: {:?}", e);
                vec![]
            }
        };

        for file in &include_files {
            // the zip file is the relative path to the report directory
            let zip_file_name = match file.strip_prefix(&self.report.dir) {
                Ok(path) => path,
                Err(_) => file.as_path(),
            };
            match self.add_file_to_zip(&file, zip_file_name.to_str().unwrap().to_string()) {
                Ok(checksum) => {
                    debug!("Checksum: {:?}", checksum);
                }
                Err(e) => error!(
                    "Failed to add file {} to zip archive: {:?}",
                    zip_file_name.display(),
                    e
                ),
            }
        }

        if let Some(writer) = self.zip_writer.take() {
            writer.finish()?;
        }

        // if encryption is disabled, we can skip the rest
        let encryption_enabled = self.report_settings.zip_archive.encryption.enabled;
        if !encryption_enabled {
            // save as encryption.json in the same directory as the output file
            self.write_encryption_metadata(&EncryptionMeta::default())?;
            return Ok(());
        }

        let algorithm = self.report_settings.zip_archive.encryption.algorithm;

        let (encrypted_key, iv, tag) = match &self.public_key {
            Some(pub_key) => {
                encrypt_evidence(&self.report.zip_path, pub_key.clone(), algorithm.clone())?
            }
            None => (vec![], vec![], vec![]),
        };

        // write metadata into json file
        let encryption_metadata = EncryptionMeta {
            version: "1.0".to_string(),
            algorithm: algorithm,
            encrypted_key: encrypted_key,
            iv: iv,
            tag: tag,
        };

        // save as encryption.json in the same directory as the output file
        self.write_encryption_metadata(&encryption_metadata)?;

        Ok(())
    }
}

pub fn read_metadata(metadata_path: &PathBuf) -> Vec<FileMeta> {
    let mut rdr = csv::Reader::from_path(metadata_path).unwrap();
    let mut file_metadata = Vec::new();
    for result in rdr.deserialize() {
        let record: FileMeta = result.unwrap();
        file_metadata.push(record);
    }
    file_metadata
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;
    use config::workflow::{ReportingMetadata, ReportingZipArchive};
    use system::SystemVariables;
    use utils::tests::Cleanup;

    fn generate_test_report(name: String, archive_enabled: bool) -> Report {
        let mut system_variables = SystemVariables::new();

        match report::Report::new(&mut system_variables, archive_enabled, name) {
            Ok(report) => report,
            Err(e) => {
                panic!("Error initializing report: {}", e);
            }
        }
    }

    #[test]
    fn test_file_processor_initialization() {
        let mut cleanup = Cleanup::new();

        let report = generate_test_report("test_file_processor_initialization".to_string(), true);
        cleanup.add(report.dir.clone());

        let file_processor: Result<FileProcessor, Box<dyn Error>> = FileProcessor::new(&report);
        assert!(
            file_processor.is_ok(),
            "Failed to initialize file processor: {:?}",
            file_processor
        );
    }

    #[test]
    fn test_file_processor_store_file() {
        let mut cleanup = Cleanup::new();

        let report = generate_test_report("test_file_processor_store_file".to_string(), true);
        cleanup.add(report.dir.clone());
        let mut file_processor = FileProcessor::new(&report).unwrap();

        let reporting_settings = Reporting {
            zip_archive: ReportingZipArchive::default(),
            metadata: ReportingMetadata::default(),
        };
        file_processor.set_report_settings(reporting_settings);

        let file_dir = cleanup.tmp_dir("test_file_processor_store_file");
        cleanup.create_files(&file_dir, vec!["test_file.txt"]);
        let file_path = file_dir.join("test_file.txt");

        let result = file_processor.store(&file_path, Some("Test Comment".to_string()));
        assert!(result.is_ok(), "Failed to store file: {:?}", result);

        let metadata_path = report.metadata_path.clone();
        let metadata = read_metadata(&metadata_path);
        assert_eq!(metadata.len(), 1, "Metadata not correctly written");

        let metadata_path = metadata[0]
            .original_path
            .strip_prefix("\\\\?\\")
            .unwrap_or(&metadata[0].original_path);

        assert_eq!(metadata_path, file_path.to_str().unwrap().to_string());
    }

    #[test]
    fn test_file_processor_add_file_to_zip() {
        let mut cleanup = Cleanup::new();

        let report = generate_test_report("test_file_processor_add_file_to_zip".to_string(), true);
        cleanup.add(report.dir.clone());

        let reporting_settings = Reporting {
            zip_archive: ReportingZipArchive::default(),
            metadata: ReportingMetadata::default(),
        };

        let mut file_processor = FileProcessor::new(&report).unwrap();
        file_processor.set_report_settings(reporting_settings);

        let file_dir = cleanup.tmp_dir("test_file_processor_add_file_to_zip");
        cleanup.create_files(&file_dir, vec!["test_file.txt"]);
        let file_path = file_dir.join("test_file.txt");

        let result = file_processor.store(&file_path, None);
        assert!(result.is_ok(), "Failed to add file to zip: {:?}", result);

        let zip_path = report.zip_path.clone();
        assert!(zip_path.exists(), "Zip file was not created");
    }

    #[test]
    fn test_file_processor_set_public_key() {
        let mut cleanup = Cleanup::new();

        let report = generate_test_report("test_file_processor_set_public_key".to_string(), true);
        cleanup.add(report.dir.clone());
        let mut file_processor = FileProcessor::new(&report).unwrap();

        let rsa = Rsa::generate(2048).unwrap();
        let public_key = rsa.public_key_to_pem().unwrap();

        file_processor.set_public_key(Rsa::public_key_from_pem(&public_key).unwrap());
        assert!(
            file_processor.public_key.is_some(),
            "Public key was not set"
        );
    }
}
