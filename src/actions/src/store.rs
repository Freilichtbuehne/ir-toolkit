use config::workflow::StoreAttributes;
use log::{debug, error, warn};
use std::path::PathBuf;
use storage::FileProcessor;
use utils::misc::get_files_by_pattern;

use super::{ActionOptions, ActionResult};

pub struct Store {}

impl Store {
    pub fn run(
        search: StoreAttributes,
        options: ActionOptions,
        file_processor: &mut FileProcessor,
    ) -> ActionResult {
        // Step 1: Split pattern string into Vec<String>
        let patterns = search.patterns.split("\n").collect::<Vec<&str>>();
        // remove empty strings
        let patterns: Vec<&str> = patterns.iter().filter(|x| !x.is_empty()).copied().collect();

        // Step 2: Search for patterns
        let mut results: Vec<PathBuf> = vec![];
        for pattern in patterns {
            let mut pattern_files = get_files_by_pattern(pattern, search.case_sensitive).unwrap();
            debug!(
                "Found {} files for pattern {:?}",
                pattern_files.len(),
                pattern
            );
            results.append(&mut pattern_files);
        }

        // Step 3: Process files
        for file in results {
            // Check if file size is within limits
            if search.size_limit != 0 {
                let file_size = match file.metadata() {
                    Ok(meta) => meta.len(),
                    Err(e) => {
                        error!("Error getting file size: {}", e);
                        continue;
                    }
                };
                if file_size > search.size_limit {
                    warn!(
                        "File {:?} is too large ({} bytes), skipping",
                        file, file_size
                    );
                    continue;
                }
            }

            match file_processor.store(&file, None) {
                Ok(_) => debug!("Stored file: {:?}", file),
                Err(e) => error!("Error storing file {:?}: {}", file.display(), e),
            }
        }

        // Step 4: Return ActionResult
        ActionResult {
            success: true,
            exit_code: Some(0),
            execution_time: options.start_time.elapsed(),
            error_message: None,
            parallel: false,
            finished: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::workflow::Reporting;
    use report::METADATA_PATH;
    use std::path::Path;
    use storage::read_metadata;
    use system::SystemVariables;
    use utils::tests::Cleanup;

    #[test]
    fn test_run_store() {
        let mut cleanup = Cleanup::new();

        let mut system_vars = SystemVariables::new();

        // initialize report
        let tite = "test".to_string();
        let report = report::Report::new(&mut system_vars, true, tite).unwrap();

        cleanup.add(report.dir.clone());

        // initialize file processor
        let mut fp = FileProcessor::new(&report).unwrap();

        // initialize report settings
        fp.set_report_settings(Reporting::default());

        // create a temp dir where files will be stored
        let temp_dir = cleanup.tmp_dir("test_run_store");

        // create files
        for file in vec!["test.txt", "test2.txt", "test.csv", "test2.csv"] {
            let file_path = temp_dir.join(file);
            let _ = std::fs::File::create(&file_path);
        }

        // create search
        let search = StoreAttributes {
            case_sensitive: false,
            patterns: temp_dir.join("*.txt").to_str().unwrap().to_string(),
            size_limit: 0,
        };

        let options = ActionOptions::default();

        let result = Store::run(search, options, &mut fp);
        assert_eq!(result.success, true);

        // load the metadata file
        let metadata_path = Path::new(&report.dir).join(METADATA_PATH);
        println!("{:?}", metadata_path);
        assert!(metadata_path.exists());
        let file_metadata = read_metadata(&metadata_path);

        // check if the two files are in the metadata vector
        assert_eq!(file_metadata.len(), 2);

        for file in vec!["test.txt", "test2.txt"] {
            let original_path = temp_dir.join(file).canonicalize().unwrap();
            let found = file_metadata.iter().any(|x| {
                let x_path = PathBuf::from(&x.original_path).canonicalize().unwrap();
                x_path == original_path
            });
            assert_eq!(found, true, "File {:?} not found in metadata", file);
        }
    }
}
