use log::debug;
use std::{error::Error, path::Path, process::Command};

pub fn run_elevated<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn Error>> {
    let cmd: Vec<&str> = vec!["sudo", path.as_ref().to_str().unwrap()];

    debug!("Running command: {:?}", cmd.join(" "));

    match Command::new(cmd[0]).args(&cmd[1..]).status() {
        Ok(status) => {
            if !status.success() {
                return Err("Failed to elevate".into());
            }
        }
        Err(e) => {
            return Err(e.to_string().into());
        }
    }

    Ok(())
}
