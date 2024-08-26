use chrono::Local;
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::{fs, io};
use system::SystemVariables;
use utils::sanitize::sanitize_dirname;

pub const ZIP_PATH: &str = "report.zip";
pub const METADATA_PATH: &str = "metadata.csv";
pub const ENCRYPTION_PATH: &str = "encryption.json";
pub const LOOT_DIR: &str = "loot_files";
pub const STORAGE_DIR: &str = "stored_files";
pub const ACTION_LOG_DIR: &str = "action_output";

#[derive(Debug)]
pub struct Report {
    pub dir: PathBuf,
    pub loot_dir: PathBuf,
    pub action_log_dir: PathBuf,
    pub zip_path: PathBuf,
    pub metadata_path: PathBuf,
    pub encryption_path: PathBuf,
    pub archive_enabled: bool,
}

impl Report {
    pub fn new(
        system_variables: &mut SystemVariables,
        archive_enabled: bool,
        name: String,
    ) -> Result<Report, io::Error> {
        // build path for report directory
        // reports/[devicename][workflowname][timestamp]

        let local_time = Local::now();
        let local_time = local_time.format("%Y-%m-%d_%H-%M-%S");

        let report_name = format!("{}_{}_{}", system_variables.device_name, name, local_time);
        let report_name = sanitize_dirname(&report_name);

        // check if reports directory exists and create it if not
        let reports_dir = system_variables.base_path.join("reports");
        if !reports_dir.exists() {
            fs::create_dir(&reports_dir).expect("Failed to create reports directory");
        }

        // create report directory
        let report_dir = reports_dir.join(&report_name);
        if report_dir.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Report directory already exists",
            ));
        }
        fs::create_dir(&report_dir)?;

        // create loot directory
        let loot_dir = report_dir.join(LOOT_DIR);
        fs::create_dir(&loot_dir)?;

        // create action log directory
        let action_log_dir = report_dir.join(ACTION_LOG_DIR);
        fs::create_dir(&action_log_dir)?;

        // if archive is disabled, create storage directory
        if !archive_enabled {
            let storage_dir = report_dir.join(STORAGE_DIR);
            fs::create_dir(&storage_dir)?;
        }

        // update system variables with current loot directory
        // each report has its own loot directory
        system_variables.loot_directory = loot_dir.clone();

        let zip_path = report_dir.join(ZIP_PATH);
        let metadata_path = report_dir.join(METADATA_PATH);
        let encryption_path = report_dir.join(ENCRYPTION_PATH);

        return Ok(Report {
            dir: report_dir,
            loot_dir,
            action_log_dir,
            zip_path,
            metadata_path,
            encryption_path,
            archive_enabled,
        });
    }

    // https://stackoverflow.com/questions/26958489/how-to-copy-a-folder-recursively-in-rust
    pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                Self::copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }
}

fn remove_dir_if_empty(dir: &Path) {
    let dir_entries = dir.read_dir();
    let mut dir_entries = match dir_entries {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read directory: {:?}", e);
            return;
        }
    };

    if dir_entries.next().is_none() {
        debug!("Removing directory: {:?}", dir);
        match fs::remove_dir(&dir) {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to remove directory: {:?}", e);
            }
        }
    }
}

impl Drop for Report {
    fn drop(&mut self) {
        // we only want to drop the report if the archive is enabled
        if !self.archive_enabled {
            return;
        }

        // delete the loot, storage, and action log directories
        let loot_dir = &self.loot_dir;
        debug!("Removing loot directory: {:?}", loot_dir);
        remove_dir_if_empty(&loot_dir);

        let action_log_dir = &self.action_log_dir;
        debug!("Removing action log directory: {:?}", action_log_dir);
        remove_dir_if_empty(&action_log_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use system::SystemVariables;
    use utils::tests::Cleanup;

    fn create_test_system_variables(name: &String, cleanup: &mut Cleanup) -> SystemVariables {
        let mut system_variables = SystemVariables::new();
        let base_path = cleanup.tmp_dir(name);
        system_variables.base_path = base_path;
        system_variables.device_name = "test_device".to_string();
        system_variables
    }

    #[test]
    fn test_report_new() {
        let mut cleanup = Cleanup::new();
        let report_name = "test_report_new".to_string();
        let mut system_variables = create_test_system_variables(&report_name, &mut cleanup);
        let report = Report::new(&mut system_variables, true, report_name.clone());

        assert!(report.is_ok(), "Report creation failed");

        let report = report.unwrap();
        assert!(report.dir.exists(), "Report directory does not exist");
        assert!(report.loot_dir.exists(), "Loot directory does not exist");
        assert!(
            report.action_log_dir.exists(),
            "Action log directory does not exist"
        );

        if report.archive_enabled {
            assert!(
                !report.dir.join(STORAGE_DIR).exists(),
                "Storage directory should not exist when archiving is enabled"
            );
        } else {
            assert!(
                report.dir.join(STORAGE_DIR).exists(),
                "Storage directory does not exist when archiving is disabled"
            );
        }

        cleanup.add(report.dir.clone());
    }

    #[test]
    fn test_report_directory_exists() {
        let mut cleanup = Cleanup::new();
        let report_name = "test_report_directory_exists".to_string();
        let mut system_variables = create_test_system_variables(&report_name, &mut cleanup);

        let report = Report::new(&mut system_variables, true, report_name.clone());
        assert!(report.is_ok(), "First report creation failed");

        let report = Report::new(&mut system_variables, true, report_name.clone());
        assert!(
            report.is_err(),
            "Report creation should fail if directory exists"
        );

        if let Err(e) = report {
            assert_eq!(e.kind(), ErrorKind::AlreadyExists, "Unexpected error kind");
        }

        cleanup.add(system_variables.base_path.join("reports").join(report_name));
    }

    #[test]
    fn test_remove_dir_if_empty() {
        let mut cleanup = Cleanup::new();
        let empty_dir = cleanup.tmp_dir("empty_dir");
        let non_empty_dir = cleanup.tmp_dir("non_empty_dir");

        cleanup.create_files(&non_empty_dir, vec!["file1.txt"]);

        remove_dir_if_empty(&empty_dir);
        assert!(!empty_dir.exists(), "Empty directory was not removed");

        remove_dir_if_empty(&non_empty_dir);
        assert!(
            non_empty_dir.exists(),
            "Non-empty directory should not be removed"
        );
    }

    #[test]
    fn test_report_drop() {
        let mut cleanup = Cleanup::new();
        let report_name = "test_report_drop".to_string();
        let mut system_variables = create_test_system_variables(&report_name, &mut cleanup);

        {
            let report = Report::new(&mut system_variables, true, report_name.clone()).unwrap();
            cleanup.add(report.dir.clone());
            assert!(
                report.loot_dir.exists(),
                "Loot directory does not exist during report lifecycle"
            );
            assert!(
                report.action_log_dir.exists(),
                "Action log directory does not exist during report lifecycle"
            );
        }

        let base_path = system_variables.base_path.join("reports").join(report_name);
        assert!(
            !base_path.join(LOOT_DIR).exists(),
            "Loot directory should be removed after report drop"
        );
        assert!(
            !base_path.join(ACTION_LOG_DIR).exists(),
            "Action log directory should be removed after report drop"
        );
    }
}
