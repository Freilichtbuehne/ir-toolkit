use config::workflow::{CustomCommand, LaunchConditions};
use log::debug;
use regex::Regex;
use std::process::Command;
use system::SystemVariables;

fn check_custom_command(custom_command: &CustomCommand, variables: &SystemVariables) -> bool {
    // replace variables in command
    let mut custom_command = custom_command.clone();
    custom_command.replace_vars(&variables.as_map());

    let args = custom_command
        .args
        .as_ref()
        .map_or(&[][..], |args| &args[..]);

    let output = Command::new(&custom_command.cmd)
        .args(args)
        .output()
        .expect("Failed to execute command");
    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if let Some(ref contains_any) = custom_command.contains_any {
        if !contains_any.iter().any(|s| result.contains(s)) {
            return false;
        }
    }

    if let Some(ref contains_all) = custom_command.contains_all {
        if !contains_all.iter().all(|s| result.contains(s)) {
            return false;
        }
    }

    if let Some(ref contains_regex) = custom_command.contains_regex {
        let re = Regex::new(contains_regex).unwrap();
        if !re.is_match(&result) {
            return false;
        }
    }

    true
}

/// Check the launch conditions of the workflow YAML
/// Returns true if all conditions are met, false otherwise
pub fn check_launch_conditions(
    condition: &mut LaunchConditions,
    variables: &SystemVariables,
) -> bool {
    // iterate over the conditions and check if they are met
    let checks: Vec<(&str, Box<dyn Fn() -> bool>)> = vec![
        ("os", Box::new(|| condition.os.contains(&variables.os))),
        (
            "enabled",
            Box::new(|| condition.enabled.map_or(true, |enabled| enabled)),
        ),
        (
            "arch",
            Box::new(|| {
                condition
                    .arch
                    .as_ref()
                    .map_or(true, |arch| arch.contains(&variables.arch))
            }),
        ),
        (
            "is_elevated",
            Box::new(|| {
                condition.is_elevated.map_or(true, |is_elevated| {
                    !is_elevated || (is_elevated && variables.is_elevated)
                })
            }),
        ),
        (
            "custom_command",
            Box::new(|| {
                condition
                    .custom_command
                    .as_ref()
                    .map_or(true, |custom_command| {
                        check_custom_command(custom_command, variables)
                    })
            }),
        ),
    ];

    // check if all conditions are met
    checks.iter().all(|(name, check)| {
        let result = check();
        if !result {
            debug!("Launch condition '{}' not met", name);
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;
    use system::SystemVariables;
    use utils::tests::Cleanup;

    #[test]
    fn test_launch_conditions_valid() {
        let yaml = if cfg!(target_os = "windows") {
            r#"
            os: ["windows", "linux", "macos"]
            enabled: true
            arch: ["x86", "x86_64", "aarch64", "arm"]
            is_elevated: false
            custom_command:
                cmd: "cmd"
                args: ["/c", "dir", "${USER_HOME}"]
                contains_any: ["test.txt"]
                contains_all: ["test", "txt"]
                contains_regex: "test.*txt"
            "#
        } else {
            r#"
            os: ["windows", "linux", "macos"]
            enabled: true
            arch: ["x86", "x86_64", "aarch64", "arm"]
            is_elevated: false
            custom_command:
                cmd: "ls"
                args: ["-l", "${USER_HOME}"]
                contains_any: ["test.txt"]
                contains_all: ["test", "txt"]
                contains_regex: "test.*txt"
            "#
        };
        let mut lc: LaunchConditions = serde_yaml::from_str(yaml).unwrap();

        let mut variables = SystemVariables::new();

        // fake the home directory and create a new file in it
        let mut cleanup = Cleanup::new();
        let home_dir = cleanup.tmp_dir("test_deserialize_launch_conditions_valid");
        let file_path = home_dir.join("test.txt");
        File::create(&file_path).unwrap();

        variables.user_home = home_dir.clone();

        let binding = variables.user_home.to_string_lossy();
        let user_home: &str = binding.as_ref();

        // assume the launch conditions is met
        assert_eq!(check_launch_conditions(&mut lc, &variables), true);

        lc.custom_command
            .as_mut()
            .unwrap()
            .replace_vars(&variables.as_map());

        assert_eq!(lc.os, vec!["windows", "linux", "macos"]);
        assert_eq!(lc.enabled.unwrap(), true);
        assert_eq!(lc.arch.unwrap(), vec!["x86", "x86_64", "aarch64", "arm"]);
        assert_eq!(lc.is_elevated.unwrap(), false);
        assert_eq!(
            lc.custom_command.as_ref().unwrap().cmd,
            if cfg!(target_os = "windows") {
                "cmd"
            } else {
                "ls"
            }
        );
        assert_eq!(
            lc.custom_command.as_ref().unwrap().args.clone().unwrap(),
            if cfg!(target_os = "windows") {
                vec!["/c", "dir", user_home]
            } else {
                vec!["-l", user_home]
            }
        );
        assert_eq!(
            lc.custom_command
                .as_ref()
                .unwrap()
                .contains_any
                .clone()
                .unwrap(),
            vec!["test.txt"]
        );
        assert_eq!(
            lc.custom_command
                .as_ref()
                .unwrap()
                .contains_all
                .clone()
                .unwrap(),
            vec!["test", "txt"]
        );
        assert_eq!(
            lc.custom_command
                .as_ref()
                .unwrap()
                .contains_regex
                .clone()
                .unwrap(),
            "test.*txt"
        );
    }
}
