pub mod binary;
pub mod command;
pub mod store;
pub mod terminal;
pub mod yara;

use core::fmt;
use std::time::{self, Duration};
pub struct ActionOptions {
    pub timeout: i32,
    pub parallel: bool,
    pub start_time: time::Instant,
}

impl Default for ActionOptions {
    fn default() -> ActionOptions {
        ActionOptions {
            timeout: 0,
            parallel: false,
            start_time: time::Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct ActionResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub execution_time: Duration,
    pub error_message: Option<String>,
    pub parallel: bool,
    pub finished: bool,
}

impl Default for ActionResult {
    fn default() -> ActionResult {
        ActionResult {
            success: true,
            exit_code: None,
            execution_time: time::Duration::from_secs(0),
            error_message: None,
            parallel: false,
            finished: false,
        }
    }
}

impl fmt::Display for ActionResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Success: {}\nExit code: {}\nExecution time: {:?}",
            self.success,
            match self.exit_code {
                Some(code) => code.to_string(),
                None => "None".to_string(),
            },
            self.execution_time,
        )?;

        if !self.success || self.error_message.is_some() {
            write!(
                f,
                "\nError message: {:?}",
                self.error_message.as_ref().unwrap_or(&"None".to_string())
            )?;
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! error_result {
    ($msg:expr) => {
        ActionResult {
            success: false,
            exit_code: Some(-1),
            execution_time: std::time::Duration::from_secs(0),
            error_message: Some($msg.to_string()),
            parallel: false,
            finished: true,
        }
    };
    ($msg:expr, $start_time:expr) => {
        ActionResult {
            success: false,
            exit_code: Some(-1),
            execution_time: $start_time.elapsed(),
            error_message: Some($msg.to_string()),
            parallel: false,
            finished: true,
        }
    };
}

#[macro_export]
macro_rules! waiting_result {
    () => {
        ActionResult {
            success: true,
            exit_code: None,
            execution_time: std::time::Duration::from_secs(0),
            error_message: None,
            parallel: true,
            finished: false,
        }
    };
}

#[macro_export]
macro_rules! get_stream_error {
    ($task:expr, $default:expr) => {
        match $task {
            Some(task) => match task.await {
                Ok(mut err) => {
                    err = err.replace("\r\n", "\n");
                    err.truncate(200);
                    Some(err)
                }
                Err(_) => Some($default.to_string()),
            },
            None => Some($default.to_string()),
        }
    };
}
