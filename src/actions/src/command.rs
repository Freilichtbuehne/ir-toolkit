use super::{error_result, get_stream_error, ActionOptions, ActionResult};
use config::workflow::CommandAttributes;
use log::debug;
use process_wrap::tokio::*;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::fs::File;
use tokio::process::Command;
use tokio::time::timeout;
use utils::process::{print_stream, read_stream};

pub struct ShellCommand {}

impl ShellCommand {
    pub async fn run(
        command: CommandAttributes,
        options: ActionOptions,
        out_file: Option<PathBuf>,
    ) -> ActionResult {
        if command.args.is_empty() {
            debug!("Executing command: {:?}", command.cmd);
        } else {
            debug!(
                "Executing command: {:?} with args: {}",
                command.cmd,
                command.args.join(" ")
            );
        };

        let mut cmd = Command::new(&command.cmd);
        cmd.args(&command.args);

        // check if cwd is set (not empty String)
        if !command.cwd.is_empty() {
            // convert cwd to PathBuf
            let cwd = PathBuf::from(&command.cwd);
            // check if cwd exists
            if !cwd.exists() {
                return error_result!(
                    format!("Specified cwd does not exist: {:?}", command.cwd).to_string()
                );
            }
            cmd.current_dir(cwd);
        }

        let output_to_console = !command.log_to_file && !options.parallel;

        if out_file.is_some() {
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

        assert_ne!(options.parallel && !command.log_to_file, true);

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
                return error_result!("Command timed out", options.start_time);
            }
        };

        let mut action_result = ActionResult::default();
        action_result.execution_time = options.start_time.elapsed();
        action_result.parallel = options.parallel;
        action_result.finished = true;
        action_result.success = output.success();
        action_result.exit_code = output.code();
        if !output.success() {
            action_result.error_message = get_stream_error!(stderr_task, "Command failed");
        }

        return action_result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::workflow::CommandAttributes;
    use ntest::timeout;
    use std::{path::PathBuf, time};
    use utils::tests::Cleanup;

    #[tokio::test]
    async fn test_run_command() {
        // set command based on OS
        let command = if cfg!(target_os = "windows") {
            CommandAttributes {
                cmd: "cmd".to_string(),
                cwd: "".to_string(),
                args: vec!["/c".to_string(), "echo".to_string(), "Hello".to_string()],
                log_to_file: false,
            }
        } else {
            CommandAttributes {
                cmd: "echo".to_string(),
                cwd: "".to_string(),
                args: vec!["Hello".to_string()],
                log_to_file: false,
            }
        };

        let options = ActionOptions::default();

        let result = ShellCommand::run(command, options, None).await;
        assert_eq!(
            result.success, true,
            "Command failed: {:?}",
            result.error_message
        );
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.error_message, None);
    }

    #[tokio::test]
    async fn test_run_command_with_output() {
        let mut cleanup = Cleanup::new();

        let command = if cfg!(target_os = "windows") {
            CommandAttributes {
                cmd: "cmd".to_string(),
                cwd: "".to_string(),
                args: vec!["/c".to_string(), "echo".to_string(), "Hello".to_string()],
                log_to_file: true,
            }
        } else {
            CommandAttributes {
                cmd: "echo".to_string(),
                cwd: "".to_string(),
                args: vec!["Hello".to_string()],
                log_to_file: true,
            }
        };

        let out_file = PathBuf::from("test_run_command_with_output.txt");
        cleanup.add(out_file.clone());

        let options = ActionOptions::default();

        let result = ShellCommand::run(command, options, Some(out_file.clone())).await;
        assert_eq!(
            result.success, true,
            "Command failed: {:?}",
            result.error_message
        );
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.error_message, None);
        assert_eq!(out_file.exists(), true);

        // check content of file
        let content = std::fs::read_to_string(out_file).unwrap();
        assert_eq!(content.contains("Hello"), true);
    }

    #[tokio::test]
    async fn test_run_command_with_error() {
        let command = if cfg!(target_os = "windows") {
            CommandAttributes {
                cmd: "cmd".to_string(),
                cwd: "".to_string(),
                args: vec!["/ccc".to_string(), "echo".to_string(), "Hello".to_string()],
                log_to_file: false,
            }
        } else {
            CommandAttributes {
                cmd: "echoooooo".to_string(),
                cwd: "".to_string(),
                args: vec!["Hello".to_string()],
                log_to_file: false,
            }
        };

        let options = ActionOptions::default();

        let result = ShellCommand::run(command, options, None).await;
        assert_eq!(result.success, false);
        assert_ne!(result.exit_code, Some(0));
        assert_eq!(result.error_message.is_some(), true);
    }

    #[tokio::test]
    async fn test_run_command_invalid_cwd() {
        let invalid_cwd = "this_path_does_not_exist";
        let command = CommandAttributes {
            cmd: "echo".to_string(),
            cwd: invalid_cwd.to_string(),
            args: vec!["Hello".to_string()],
            log_to_file: false,
        };

        let options = ActionOptions {
            timeout: 0,
            parallel: false,
            start_time: time::Instant::now(),
        };

        let result = ShellCommand::run(command, options, None).await;
        assert_eq!(result.success, false);
        assert_eq!(result.exit_code, Some(-1));
        // assert that error message contains the cwd
        let error_message = result.error_message.unwrap();
        assert_eq!(error_message.contains(invalid_cwd), true);
    }

    #[tokio::test]
    #[timeout(2000)]
    async fn test_run_command_child_procs() {
        let command = if cfg!(target_os = "windows") {
            CommandAttributes {
                cmd: "cmd".to_string(),
                cwd: "".to_string(),
                args: vec![
                    "/c".to_string(),
                    "ping".to_string(),
                    "-t".to_string(),
                    "127.0.0.1".to_string(),
                ],
                log_to_file: false,
            }
        } else {
            CommandAttributes {
                cmd: "bash".to_string(),
                cwd: "".to_string(),
                args: vec!["-c".to_string(), "sleep 10".to_string()],
                log_to_file: false,
            }
        };

        let options = ActionOptions {
            timeout: 1,
            parallel: false,
            start_time: time::Instant::now(),
        };

        let result = ShellCommand::run(command, options, None).await;

        assert_eq!(result.success, false, "Expected a timeout",);
        assert_ne!(result.exit_code, Some(0));
        assert_eq!(result.error_message, Some("Command timed out".to_string()));
    }
}
