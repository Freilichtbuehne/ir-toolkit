use super::{error_result, get_stream_error, ActionOptions, ActionResult};
use config::workflow::BinaryAttributes;
use log::debug;
use process_wrap::tokio::*;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::fs::File;
use tokio::process::Command;
use tokio::time::timeout;
use utils::process::{print_stream, read_stream};
pub struct Binary {}

impl Binary {
    pub async fn run(
        bin: BinaryAttributes,
        options: ActionOptions,
        out_file: Option<PathBuf>,
        custom_files_dir: PathBuf,
    ) -> ActionResult {
        // Case distinction:
        // 1. If bin.path is relative, search in the custom_files directory
        // 2. If bin.path is absolute, directly use it

        // first, convert to PathBuf
        let bin_path = PathBuf::from(&bin.path);
        let bin_path = match bin_path.is_absolute() {
            true => bin_path,
            false => custom_files_dir.join(bin_path),
        };

        // check if file exists
        if !bin_path.exists() {
            return error_result!(format!("File not found: {:?}", bin_path));
        }

        if bin.args.is_empty() {
            debug!("Executing binary: {}", bin_path.display());
        } else {
            debug!(
                "Executing binary: {} with args: {:?}",
                bin_path.display(),
                bin.args.join(" ")
            );
        }

        //TODO: print checksum of binary or version
        let mut cmd = Command::new(&bin_path);
        cmd.args(&bin.args);

        let output_to_console = !bin.log_to_file && !options.parallel;

        if out_file.is_some() && bin.log_to_file {
            let out_file = out_file.unwrap();
            let std_out_file = File::create(&out_file).await.unwrap();
            cmd.stdout(std_out_file.into_std().await);
            let std_err_file = File::create(&out_file).await.unwrap();
            cmd.stderr(std_err_file.into_std().await);
        } else if output_to_console {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        } else {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        }

        let mut child = TokioCommandWrap::from(cmd);
        child.wrap(KillOnDrop);
        #[cfg(windows)]
        child.wrap(JobObject);
        #[cfg(unix)]
        child.wrap(ProcessGroup::leader());

        let mut child = match child.spawn() {
            Ok(child) => child,
            Err(e) => return error_result!(e.to_string()),
        };

        let stderr_task: Option<tokio::task::JoinHandle<String>> = match output_to_console {
            true => {
                // run command in parallel and print output to console
                let stdout = child.inner_mut().stdout.take();
                let stderr = child.inner_mut().stderr.take();

                tokio::spawn(print_stream(stdout));
                Some(tokio::spawn(read_stream(stderr, true)))
            }
            false => None,
        };

        let output = if options.timeout > 0 {
            timeout(
                Duration::from_secs(options.timeout as u64),
                Box::into_pin(child.wait()),
            )
            .await
        } else {
            Ok(Box::into_pin(child.wait()).await)
        };

        let output = match output {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return error_result!(e.to_string(), options.start_time),
            Err(_) => {
                Box::into_pin(child.kill()).await.unwrap();
                return error_result!("Process timed out", options.start_time);
            }
        };

        let mut action_result = ActionResult::default();
        action_result.execution_time = options.start_time.elapsed();
        action_result.parallel = options.parallel;
        action_result.finished = true;
        action_result.success = output.success();
        action_result.exit_code = output.code();
        if !output.success() {
            action_result.error_message = get_stream_error!(stderr_task, "Process failed");
        }

        return action_result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::workflow::BinaryAttributes;
    use futures::executor::block_on;
    use std::path::PathBuf;
    use system::SystemVariables;
    use utils::tests::Cleanup;

    #[tokio::test]
    #[cfg(not(unix))]
    async fn test_run_valid_binary() {
        // choose a binary for each os
        let bin_path = match std::env::consts::OS {
            "linux" => "/bin/ls".to_string(),
            "macos" => "/bin/ls".to_string(),
            "windows" => "C:\\Windows\\System32\\cmd.exe".to_string(),
            _ => panic!("Unsupported OS"),
        };

        let mut cleanup = Cleanup::new();
        let out_file = PathBuf::from("test_run_valid_binary.txt");
        cleanup.add(out_file.clone());

        let bin = BinaryAttributes {
            path: bin_path,
            args: vec![],
            log_to_file: true,
        };

        let system_vars = SystemVariables::new();
        let options = ActionOptions::default();

        let result = block_on(Binary::run(
            bin,
            options,
            Some(out_file.clone()),
            system_vars.custom_files_directory,
        ));

        assert_eq!(result.success, true);

        // check if file was created and not empty
        assert_eq!(out_file.exists(), true);

        let file_content = std::fs::read_to_string(&out_file).unwrap();
        assert_eq!(file_content.is_empty(), false);
    }

    #[tokio::test]
    async fn test_run_invalid_binary() {
        let mut cleanup = Cleanup::new();
        // create a broken binary that cannot be executed+
        let binary = PathBuf::from("broken_binary");
        cleanup.add(binary.clone());

        // create the file and write some content
        let content = "This is not a binary";
        std::fs::write(&binary, content).unwrap();

        // make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&binary).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&binary, perms).unwrap();
        }

        let bin = BinaryAttributes {
            path: binary.to_str().unwrap().to_string(),
            args: vec![],
            log_to_file: false,
        };

        let system_vars = SystemVariables::new();
        let options = ActionOptions::default();
        let result = block_on(Binary::run(
            bin,
            options,
            None,
            system_vars.custom_files_directory,
        ));

        assert_eq!(result.success, false);

        // check if error message is not empty
        assert_eq!(result.error_message.is_none(), false);
    }
}
