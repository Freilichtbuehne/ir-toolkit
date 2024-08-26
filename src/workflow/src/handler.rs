use crate::{launch_conditions::check_launch_conditions, runner};
use crypto::load_public_key;
use log::{debug, error, info};
use std::path::PathBuf;
use storage::FileProcessor;
use system::SystemVariables;
use utils::misc::get_files_by_patterns;

pub const WORKFLOWS_DIR: &str = "workflows";

pub struct WorkflowHandler {
    workflow_files: Vec<PathBuf>,
    system_variables: SystemVariables,
}

impl WorkflowHandler {
    pub fn init(system_variables: SystemVariables) -> Self {
        Self {
            workflow_files: WorkflowHandler::get_workflow_files(&system_variables.base_path),
            system_variables: system_variables,
        }
    }

    pub fn run(&mut self) {
        // error if no workflow files are found
        if self.workflow_files.is_empty() {
            error!("No workflow files found.");
            return;
        }

        // iterate over all workflow files
        for file in &self.workflow_files {
            debug!("Reading workflow file: {}", file.display());
            let mut workflow = match runner::Workflow::init(file) {
                Ok(workflow) => workflow,
                Err(_) => {
                    error!("Error initializing workflow for file: {}", file.display());
                    continue;
                }
            };

            // check launch conditions
            if !check_launch_conditions(
                &mut workflow.runner.launch_conditions,
                &mut self.system_variables,
            ) {
                debug!("Launch conditions not met for file: {}", file.display());
                continue;
            }

            // initialize report
            let tite = workflow.runner.properties.get("title").unwrap().to_string();
            let archive_enabled = workflow.runner.reporting.zip_archive.enabled;
            let report =
                match report::Report::new(&mut self.system_variables, archive_enabled, tite) {
                    Ok(report) => report,
                    Err(e) => {
                        error!("Error initializing report for {:?}: {}", file, e);
                        continue;
                    }
                };

            // initialize file processor
            let mut fp = match FileProcessor::new(&report) {
                Ok(fp) => fp,
                Err(e) => {
                    error!("Error initializing file processor for {:?}: {}", file, e);
                    continue;
                }
            };

            fp.set_report_settings(workflow.runner.reporting.clone());

            // reporting
            let encryption_settings = &workflow.runner.reporting.zip_archive.encryption;
            if encryption_settings.enabled {
                // convert public key filename to PathBuf (e.g. public.pem)
                let public_key_path = PathBuf::from(&encryption_settings.public_key);
                // prepend base path + /keys to public key filename
                let public_key_path = self
                    .system_variables
                    .base_path
                    .join("keys")
                    .join(public_key_path);

                info!("Loading public key: {}", public_key_path.to_string_lossy());
                if let Ok(public_key) = load_public_key(public_key_path.clone()) {
                    fp.set_public_key(public_key);
                } else {
                    error!(
                        "Error loading public key: {}",
                        public_key_path.to_string_lossy()
                    );
                    continue;
                }
            }

            // run the workflow
            if let Err(_) = workflow.run(&report, &self.system_variables, &mut fp) {
                error!("Error running workflow for file: {}", file.display());
            }

            // finish the file processor
            match fp.finish() {
                Ok(_) => (),
                Err(e) => error!("Error finishing file processor: {}", e),
            }
        }
    }

    pub fn get_workflow_files(base_path: &PathBuf) -> Vec<PathBuf> {
        let patterns = vec![
            format!(
                "{}/{}/**/*.yaml",
                base_path.to_string_lossy(),
                WORKFLOWS_DIR
            ),
            format!("{}/{}/**/*.yml", base_path.to_string_lossy(), WORKFLOWS_DIR),
        ];

        match get_files_by_patterns(patterns, false) {
            Ok(files) => files,
            Err(e) => {
                error!("Error getting files by pattern: {}", e);
                return Vec::new();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utils::tests::Cleanup;

    #[test]
    fn test_get_workflow_files() {
        let mut cleanup = Cleanup::new();
        let tmp_dir = cleanup.tmp_dir("test_get_workflow_files");

        // create workflow directory structure
        let files = vec![
            "workflows/workflow.yaml",
            "workflows/workflow.yml",
            "workflows/subdir/workflow.yml",
            "workflows/subdir/workflow.yaml",
            "workflows/subdir/subdir/xyz.yml",
        ];
        cleanup.create_files(&tmp_dir, files);

        let workflow_files = WorkflowHandler::get_workflow_files(&tmp_dir);

        // assert that all files are found
        assert_eq!(workflow_files.len(), 5, "Did not find all workflow files");
    }
}
