#[cfg(test)]
mod tests {
    use crate::*;
    use core::panic;
    use crypto::load_public_key;
    use fs::File;
    use report::{Report, STORAGE_DIR};
    use std::io::{BufWriter, Seek, Write};
    use std::path::PathBuf;
    use storage::FileProcessor;
    use system::{get_base_path, SystemVariables};
    use utils::{misc::get_files_by_pattern, tests::Cleanup};
    use workflow::runner::Workflow;
    use zip::write::{ExtendedFileOptions, FileOptions};
    use zip::ZipWriter;

    fn generate_test_report(tmp_dir: PathBuf, workflow: String, name: String) -> Report {
        // write the workflow file
        let workflow_file_path = tmp_dir.join("workflow.yml");
        match std::fs::File::create(&workflow_file_path) {
            Ok(mut file) => {
                if let Err(_) = file.write_all(workflow.as_bytes()) {
                    panic!("Error writing workflow file");
                }
            }
            Err(_) => {
                panic!("Error creating workflow file");
            }
        }

        let mut workflow = match Workflow::init(&workflow_file_path) {
            Ok(workflow) => workflow,
            Err(e) => {
                panic!("Error initializing workflow: {}", e);
            }
        };

        // get system variables
        let mut system_variables = SystemVariables::new();

        // initialize report
        let archive_enabled = workflow.runner.reporting.zip_archive.enabled;
        let report = match report::Report::new(&mut system_variables, archive_enabled, name) {
            Ok(report) => report,
            Err(e) => {
                panic!("Error initializing report: {}", e);
            }
        };

        // initialize file processor
        let mut fp = match FileProcessor::new(&report) {
            Ok(fp) => fp,
            Err(e) => {
                panic!("Error initializing file processor: {}", e);
            }
        };

        fp.set_report_settings(workflow.runner.reporting.clone());

        // reporting
        let encryption_settings = &workflow.runner.reporting.zip_archive.encryption;
        if encryption_settings.enabled {
            // convert public key filename to PathBuf (e.g. public.pem)
            let public_key_path = PathBuf::from(&encryption_settings.public_key);
            // prepend base path + /keys to public key filename
            let public_key_path = system_variables
                .base_path
                .join("keys")
                .join(public_key_path);

            info!("Loading public key: {}", public_key_path.to_string_lossy());
            if let Ok(public_key) = load_public_key(public_key_path.clone()) {
                fp.set_public_key(public_key);
            } else {
                panic!("Error loading public key");
            }
        }

        // run the workflow
        if let Err(_) = workflow.run(&report, &system_variables, &mut fp) {
            panic!("Error running workflow");
        }

        // finish the file processor
        match fp.finish() {
            Ok(_) => (),
            Err(e) => {
                panic!("Error finishing file processor: {}", e);
            }
        }

        report
    }

    #[test]
    fn check_unpack_archived() {
        // Create some test files to store
        let mut cleanup = Cleanup::new();
        let tmp_dir = cleanup.tmp_dir("check_unpack_archived");
        let tmp_files = vec![tmp_dir.join("test.txt"), tmp_dir.join("test.csv")];
        for file in &tmp_files {
            let _ = std::fs::File::create(file);
        }

        // define a workflow file
        let workflow_file = format!(
            r#"
            properties:
              title: "test"
              description: "test"
              author: "test"
              version: "1.0"
            launch_conditions:
              os: ["windows", "linux", "macos"]
              arch: ["x86", "x86_64", "aarch64", "arm"]
              is_elevated: false
            options:
              time_zone: "Europe/Berlin"
            actions:
              - name: run_command
                type: command
                attributes:
                  cmd: "{}"
                  args: ["echo", "test"]
                  log_to_file: true
              - name: store_file
                type: store
                attributes:
                  patterns: |
                    {}/*
            workflow:
              - action: store_file
              - action: run_command
            reporting:
              zip_archive:
                enabled: true
                encryption:
                  enabled: true
                  public_key: "example_public.pem"
                  algorithm: CHACHA20-POLY1305
                compression:
                  enabled: true
                  size_limit: "100 MB"
              metadata:
                mac_times: true
                checksums: true
                paths: true
        "#,
            match cfg!(windows) {
                true => "cmd",
                false => "sh",
            },
            tmp_dir.to_str().unwrap()
        );

        let report = generate_test_report(
            tmp_dir.clone(),
            workflow_file,
            "test_check_unpack_archived".to_string(),
        );

        // Add report path to cleanup
        cleanup.add(report.dir.clone());

        // Run the unpacker
        let matches = get_command().get_matches_from(vec![
            "unpacker",
            "-i",
            report.dir.to_str().unwrap(),
            "-k",
            get_base_path()
                .join("keys")
                .join("example_private.pem")
                .to_str()
                .unwrap(),
            "--verify",
            "--restore",
        ]);

        if let Err(e) = run(matches) {
            panic!("Unpacker failed: {}", e);
        }

        // Get report directory and drop it
        let report_dir = report.dir.clone();
        drop(report);

        // Verify the output
        let output_dir = report_dir.join("output");
        assert!(
            output_dir.exists(),
            "Output directory does not exist: {:?}",
            output_dir
        );

        let storage_dir = output_dir.join(STORAGE_DIR);
        assert!(
            storage_dir.exists(),
            "Storage directory does not exist: {:?}",
            storage_dir
        );

        // search for the files in the output directory and subdirectories
        let pattern = format!("{}/**/*", storage_dir.to_str().unwrap());
        let matched_files = get_files_by_pattern(&pattern, true).unwrap();

        // check if we can find the tmp_files
        for file in &tmp_files {
            let storage_location =
                path_to_storage_location(&file.to_str().unwrap().to_string(), &output_dir);
            // check if the file exists in the output directory
            assert_eq!(
                storage_location.exists(),
                true,
                "File {:?} not found in output directory",
                storage_location.to_str().unwrap()
            );
            // double check if the any of matched_files ends with the storage locations filename
            assert_eq!(
                matched_files
                    .iter()
                    .any(|f| f.ends_with(&storage_location.file_name().unwrap())),
                true,
                "File {:?} not found in matched files",
                file.to_str().unwrap()
            );
        }
    }

    #[test]
    fn check_unpack_archived_tampered() {
        // Create some test files to store
        let mut cleanup = Cleanup::new();
        let tmp_dir = cleanup.tmp_dir("check_unpack_archived_tampered");
        let tmp_files = vec![tmp_dir.join("test.txt"), tmp_dir.join("test.csv")];
        for file in &tmp_files {
            let _ = std::fs::File::create(file);
        }

        // define a workflow file
        let workflow_file = format!(
            r#"
            properties:
              title: "test"
              description: "test"
              author: "test"
              version: "1.0"
            launch_conditions:
              os: ["windows", "linux", "macos"]
              arch: ["x86", "x86_64", "aarch64", "arm"]
              is_elevated: false
            options:
              time_zone: "Europe/Berlin"
            actions:
              - name: store_file
                type: store
                attributes:
                  patterns: |
                    {}/*
            workflow:
              - action: store_file
            reporting:
              zip_archive:
                enabled: true
                encryption:
                  enabled: true
                  public_key: "example_public.pem"
                  algorithm: CHACHA20-POLY1305
                compression:
                  enabled: true
                  size_limit: "100 MB"
              metadata:
                mac_times: true
                checksums: true
                paths: true
        "#,
            tmp_dir.to_str().unwrap()
        );

        let report = generate_test_report(
            tmp_dir.clone(),
            workflow_file,
            "test_check_unpack_archived_tampered".to_string(),
        );

        // add report path to cleanup
        cleanup.add(report.dir.clone());

        // modify the archive by replacing the last byte with a different value
        let archive_path = report.dir.join("report.zip");
        let mut archive = std::fs::OpenOptions::new()
            .write(true)
            .open(&archive_path)
            .unwrap();
        archive.seek(std::io::SeekFrom::End(-1)).unwrap();
        archive.write_all(&[0x00]).unwrap();

        // run the unpacker
        let matches = get_command().get_matches_from(vec![
            "unpacker",
            "-i",
            report.dir.to_str().unwrap(),
            "-k",
            get_base_path()
                .join("keys")
                .join("example_private.pem")
                .to_str()
                .unwrap(),
            "--verify",
            "--restore",
        ]);

        // assert that the unpacker fails with an error
        let result = run(matches);
        assert!(result.is_err(), "Unpacker should have failed");
    }

    #[test]
    fn check_unpack_not_archived() {
        // Create some test files to store
        let mut cleanup = Cleanup::new();
        let tmp_dir = cleanup.tmp_dir("check_unpack_not_archived");
        let tmp_files = vec![tmp_dir.join("test.txt"), tmp_dir.join("test.csv")];
        for file in &tmp_files {
            let _ = std::fs::File::create(file);
        }

        // define a workflow file
        let workflow_file = format!(
            r#"
            properties:
              title: "test"
              description: "test"
              author: "test"
              version: "1.0"
            launch_conditions:
              os: ["windows", "linux", "macos"]
              arch: ["x86", "x86_64", "aarch64", "arm"]
              is_elevated: false
            options:
              time_zone: "Europe/Berlin"
            actions:
              - name: run_command
                type: command
                attributes:
                  cmd: "{}"
                  args: ["echo", "test"]
                  log_to_file: true
              - name: store_file
                type: store
                attributes:
                  patterns: |
                    {}/*
            workflow:
              - action: store_file
              - action: run_command
            reporting:
              zip_archive:
                enabled: false
                encryption:
                  enabled: true
                  public_key: "example_public.pem"
                  algorithm: CHACHA20-POLY1305
                compression:
                  enabled: true
                  size_limit: "100 MB"
              metadata:
                mac_times: true
                checksums: true
                paths: true
        "#,
            match cfg!(windows) {
                true => "cmd",
                false => "sh",
            },
            tmp_dir.to_str().unwrap()
        );

        let report = generate_test_report(
            tmp_dir.clone(),
            workflow_file,
            "test_check_unpack_not_archived".to_string(),
        );

        // Add report path to cleanup
        cleanup.add(report.dir.clone());

        // Run the unpacker
        let matches = get_command().get_matches_from(vec![
            "unpacker",
            "-i",
            report.dir.to_str().unwrap(),
            "-k",
            get_base_path()
                .join("keys")
                .join("example_private.pem")
                .to_str()
                .unwrap(),
            "--verify",
            "--restore",
        ]);

        if let Err(e) = run(matches) {
            panic!("Unpacker failed: {}", e);
        }

        // Verify the output
        let storage_dir = report.dir.join(STORAGE_DIR);
        assert!(
            storage_dir.exists(),
            "Storage directory does not exist: {:?}",
            storage_dir
        );

        // search for the files in the output directory and subdirectories
        let pattern = format!("{}/**/*", storage_dir.to_str().unwrap());
        let matched_files = get_files_by_pattern(&pattern, true).unwrap();

        // check if we can find the tmp_files
        for file in &tmp_files {
            let storage_location =
                path_to_storage_location(&file.to_str().unwrap().to_string(), &report.dir);
            // check if the file exists in the output directory
            assert_eq!(
                storage_location.exists(),
                true,
                "File {:?} not found in output directory",
                storage_location.to_str().unwrap()
            );
            // double check if the any of matched_files ends with the storage locations filename
            assert_eq!(
                matched_files
                    .iter()
                    .any(|f| f.ends_with(&storage_location.file_name().unwrap())),
                true,
                "File {:?} not found in matched files",
                file.to_str().unwrap()
            );
        }
    }

    #[test]
    fn check_encryption_detection() {
        let mut cleanup = Cleanup::new();

        // Create a zip archive
        let zip_path = PathBuf::from("check_encryption_detection.zip");
        cleanup.add(zip_path.clone());
        File::create(&zip_path).unwrap();

        let file = std::fs::File::create(&zip_path).expect("Failed to create zip file");
        let mut zip_writer = ZipWriter::new(BufWriter::new(file));

        // create a dummy dir inside the zip archive
        let file_options: FileOptions<ExtendedFileOptions> = FileOptions::default();
        zip_writer.add_directory("test", file_options).unwrap();

        // write the zip archive
        zip_writer.finish().unwrap();

        assert_eq!(is_valid_zip_archive(&zip_path), true);
    }
}
