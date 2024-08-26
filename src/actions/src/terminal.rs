use super::{error_result, get_stream_error, ActionOptions, ActionResult};
use config::workflow::TerminalAttributes;
use log::{debug, info, warn};
use process_wrap::tokio::*;
use std::{path::PathBuf, process::Stdio, time};
use tokio::process::Command;
use utils::process::read_stream;

pub struct Terminal {}

#[cfg(windows)]
fn get_windows_command(
    shell: String,
    out_file: Option<PathBuf>,
    terminal: &TerminalAttributes,
) -> Vec<String> {
    let mut base = match terminal.separate_window {
        true => vec!["conhost".to_string()],
        false => vec![],
    };

    let mut appendix = match terminal.enable_transcript {
        true => vec![
            "powershell".to_string(),
            "-Command".to_string(),
            format!(
                "Start-Transcript -Force -Path {}; {}",
                out_file.unwrap().display(),
                shell
            ),
        ],
        false => vec![shell],
    };

    base.append(&mut appendix);
    base
}

#[cfg(target_os = "macos")]
fn get_macos_command(
    shell: String,
    out_file: Option<PathBuf>,
    terminal: &TerminalAttributes,
) -> Vec<String> {
    let base = if terminal.enable_transcript {
        // See: https://www.unix.com/man-page/osx/1/script/
        format!("script -a {} {}", out_file.unwrap().display(), shell)
    } else {
        shell.clone()
    };

    match terminal.separate_window {
        true => vec![
            "osascript".to_string(),
            "-e".to_string(),
            format!("'tell application \"Terminal\" to do script \"{}\"'", base),
        ],
        false => vec![shell],
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn get_unix_command(
    shell: String,
    out_file: Option<PathBuf>,
    terminal: &TerminalAttributes,
) -> Vec<String> {
    let base_command = if terminal.enable_transcript {
        // See: https://man7.org/linux/man-pages/man1/script.1.html
        format!("script -c '{}' {}", shell, out_file.unwrap().display())
    } else {
        shell
    };

    let fallback = vec!["sh".to_string(), "-c".to_string(), base_command.clone()];

    if terminal.separate_window {
        // Define the commands for different terminal emulators
        let commands = vec![
            (
                "gnome-terminal",
                vec!["--wait", "--", "bash", "-c", &base_command],
            ),
            ("konsole", vec!["--noclose", "-e", &base_command]),
            ("xfce4-terminal", vec!["--hold", "-e", &base_command]),
            ("lxterminal", vec!["-e", &base_command]),
            ("terminology", vec!["-e", &base_command]),
            ("xterm", vec!["-hold", "-e", &base_command]),
        ];

        // Find the first available terminal emulator
        for (term, args) in commands {
            if std::process::Command::new("which")
                .arg(term)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
            {
                let mut command = vec![term.to_string()];
                command.extend(args.iter().map(|&s| s.to_string()));
                return command;
            }
        }

        // Fallback to running the script in the current terminal
        fallback
    } else {
        fallback
    }
}

impl Terminal {
    pub async fn run(
        terminal: TerminalAttributes,
        options: ActionOptions,
        out_file: Option<PathBuf>,
    ) -> ActionResult {
        // Determine the shell to use
        let shell = Terminal::get_shell(&terminal.shell);

        // Determine the command to run
        let cmd = Terminal::build_command(shell, out_file, &terminal);

        // error check
        let cmd = match cmd {
            Some(cmd) => cmd,
            None => {
                return error_result!("Failed to determine the shell command");
            }
        };

        if !terminal.separate_window {
            info!("Type 'exit' to exit the terminal session");
        }

        let mut child = TokioCommandWrap::from(cmd);
        if terminal.wait {
            child.wrap(KillOnDrop);
        }
        #[cfg(windows)]
        child.wrap(JobObject);
        //#[cfg(unix)]
        //child.wrap(ProcessGroup::leader());
        let mut child = match child.spawn() {
            Ok(child) => child,
            Err(e) => return error_result!(e.to_string()),
        };

        // If wait is false, we run the command in the background
        if !terminal.wait {
            return ActionResult {
                success: true,
                exit_code: Some(0),
                execution_time: time::Duration::new(0, 0),
                error_message: None,
                parallel: options.parallel,
                finished: true,
            };
        }

        let stderr = child.inner_mut().stderr.take();
        let stderr_task: Option<tokio::task::JoinHandle<String>> =
            Some(tokio::spawn(read_stream(stderr, false)));

        // If wait is true, we wait for the command to finish
        let output = match Box::into_pin(child.wait()).await {
            Ok(output) => output,
            Err(e) => return error_result!(e.to_string(), options.start_time),
        };

        ActionResult {
            success: output.success(),
            exit_code: Some(output.code().unwrap()),
            execution_time: options.start_time.elapsed(),
            error_message: match output.success() {
                true => None,
                false => get_stream_error!(stderr_task, "Terminal failed"),
            },
            parallel: options.parallel,
            finished: true,
        }
    }

    pub fn get_shell(shell: &String) -> String {
        // check if a shell is specified
        if !shell.is_empty() {
            return shell.clone();
        }

        return if cfg!(windows) {
            // check if powershell is available
            match std::process::Command::new("powershell").arg("-?").output() {
                Ok(output) if output.status.success() => "powershell".to_string(),
                _ => "cmd".to_string(),
            }
        } else if cfg!(target_os = "macos") {
            // on macOS Catalina, the default shell is now zsh
            // on older versions, it's bash
            match std::env::var("SHELL") {
                Ok(shell) if shell.contains("zsh") => shell,
                Ok(shell) if shell.contains("bash") => shell,
                _ => "sh".to_string(),
            }
        } else if cfg!(unix) {
            // if on unix, check the SHELL environment variable
            match std::env::var("SHELL") {
                Ok(shell) => shell,
                Err(_) => "bash".to_string(),
            }
        } else {
            warn!("Unknown OS, defaulting to sh");
            "sh".to_string()
        };
    }

    fn build_command(
        shell: String,
        out_file: Option<PathBuf>,
        terminal: &TerminalAttributes,
    ) -> Option<Command> {
        #[cfg(windows)]
        let command = get_windows_command(shell, out_file, terminal);
        #[cfg(target_os = "macos")]
        let command = get_macos_command(shell, out_file, terminal);
        #[cfg(all(unix, not(target_os = "macos")))]
        let command = get_unix_command(shell, out_file, terminal);
        #[cfg(not(any(windows, target_os = "macos", unix)))]
        let command = vec![];

        // If command is empty, return an error
        if command.is_empty() {
            return None;
        }

        debug!("Launching interactive shell: {:?}", command.join(" "));

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);

        // determine the stdio configuration
        if terminal.separate_window {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd.stdin(Stdio::piped());
        } else if terminal.wait && !terminal.separate_window {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
            cmd.stdin(Stdio::inherit());
        }

        Some(cmd)
    }
}

#[cfg(test)]
mod tests {

    use crate::*;

    use config::workflow::TerminalAttributes;
    use std::process::Stdio;
    use terminal::Terminal;
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;
    use utils::tests::Cleanup;

    #[tokio::test]
    async fn test_shell() {
        let shell = Terminal::get_shell(&"".to_string());
        // execute the shell, and check if the process starts
        // if the shell is not found, the test will fail
        let success = Command::new(&shell).status().await.unwrap().success();
        assert_eq!(success, true);
    }

    #[tokio::test]
    async fn test_integrated_terminal() {
        let terminal = TerminalAttributes {
            shell: "".to_string(),
            separate_window: false,
            enable_transcript: false,
            wait: true,
        };

        let shell = Terminal::get_shell(&terminal.shell);
        assert_eq!(shell.is_empty(), false);

        let cmd = Terminal::build_command(shell, None, &terminal);

        // run the command, send "echo hello world" to the shell and check if the output contains "hello world"
        let mut cmd = cmd.unwrap();
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());

        let mut child = cmd.spawn().unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"echo hello world\nexit\n")
            .await
            .unwrap();

        let output = child.wait_with_output().await.unwrap();
        assert_eq!(output.status.success(), true);

        let stdout = String::from_utf8_lossy(&output.stdout);

        // check if the output contains "hello world"
        assert_eq!(stdout.contains("hello world"), true);
    }

    #[tokio::test]
    async fn test_integrated_terminal_transcript() {
        let terminal = TerminalAttributes {
            shell: "".to_string(),
            separate_window: false,
            enable_transcript: true,
            wait: true,
        };

        let mut cleanup = Cleanup::new();
        let dir = cleanup.tmp_dir("test_integrated_terminal_transcript");
        let file_path = dir.join("transcript.log");

        let shell = Terminal::get_shell(&terminal.shell);
        assert_eq!(shell.is_empty(), false);

        let cmd = Terminal::build_command(shell, Some(file_path.clone()), &terminal);

        // run the command, send "echo hello world" to the shell and check if the output contains "hello world"
        let mut cmd = cmd.unwrap();
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());

        let mut child = cmd.spawn().unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"echo hello world\nexit\n")
            .await
            .unwrap();

        let output = child.wait_with_output().await.unwrap();
        assert_eq!(output.status.success(), true);

        // check if the transcript file exists
        assert_eq!(file_path.exists(), true);

        // check if the transcript file is not empty
        let transcript = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(transcript.is_empty(), false);
    }
}
