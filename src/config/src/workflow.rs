use byte_unit::Byte;
use humantime::parse_duration;
use log::{error, warn};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use std::{error::Error, fs::File};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomCommand {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub contains_any: Option<Vec<String>>,
    pub contains_all: Option<Vec<String>>,
    pub contains_regex: Option<String>,
}

impl CustomCommand {
    pub fn replace_vars(&mut self, variables: &HashMap<String, String>) {
        let cloned_self = self.clone();
        let value = serde_yaml::to_value(cloned_self).unwrap();
        let updated_value = replace_in_value(value, variables);
        *self = serde_yaml::from_value(updated_value).unwrap();
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LaunchConditions {
    pub os: Vec<String>,
    pub enabled: Option<bool>,
    pub arch: Option<Vec<String>>,
    pub is_elevated: Option<bool>,
    pub custom_command: Option<CustomCommand>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum ActionType {
    #[serde(rename = "binary")]
    Binary,
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "store")]
    Store,
    #[serde(rename = "yara")]
    Yara,
    #[serde(rename = "terminal")]
    Terminal,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ActionType::Binary => write!(f, "binary"),
            ActionType::Command => write!(f, "command"),
            ActionType::Store => write!(f, "store"),
            ActionType::Yara => write!(f, "yara"),
            ActionType::Terminal => write!(f, "terminal"),
        }
    }
}

// only some action types are able to run in parallel
fn parallel_action_types() -> Vec<ActionType> {
    vec![
        ActionType::Binary,
        ActionType::Command,
        ActionType::Terminal,
    ]
}

// only some action typed support a timeout
fn timeout_action_types() -> Vec<ActionType> {
    vec![ActionType::Binary, ActionType::Command]
}

fn default_case_sensitive() -> bool {
    false
}

fn default_size_limit() -> u64 {
    0
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreAttributes {
    #[serde(default = "default_case_sensitive")]
    pub case_sensitive: bool,
    pub patterns: String,
    #[serde(default = "default_size_limit")]
    #[serde(deserialize_with = "deserialize_size_limit")]
    #[serde(serialize_with = "serialize_size_limit")]
    pub size_limit: u64,
}

fn default_args() -> Vec<String> {
    Vec::new()
}

fn default_log_to_file() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BinaryAttributes {
    pub path: String,
    #[serde(default = "default_args")]
    pub args: Vec<String>,
    #[serde(default = "default_log_to_file")]
    pub log_to_file: bool,
}

fn default_cwd() -> String {
    String::new()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandAttributes {
    pub cmd: String,
    #[serde(default = "default_args")]
    pub args: Vec<String>,
    #[serde(default = "default_cwd")]
    pub cwd: String,
    #[serde(default = "default_log_to_file")]
    pub log_to_file: bool,
}

fn default_store_on_match() -> bool {
    true
}

fn default_threads() -> u32 {
    1
}

fn default_scan_timeout() -> i32 {
    60
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct YaraAttributes {
    pub rules_paths: String,
    pub files_to_scan: String,
    #[serde(default = "default_store_on_match")]
    pub store_on_match: bool,
    #[serde(default = "default_threads")]
    pub num_threads: u32,
    #[serde(default = "default_scan_timeout")]
    #[serde(deserialize_with = "deserialize_timeout")]
    #[serde(serialize_with = "serialize_timeout")]
    pub scan_timeout: i32,
}

fn deserialize_timeout<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;

    match parse_duration(&s) {
        Ok(duration) => Ok(duration.as_secs() as i32),
        Err(_) => Err(serde::de::Error::custom("Invalid duration")),
    }
}

fn serialize_timeout<S>(value: &i32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let duration = std::time::Duration::from_secs(*value as u64);
    let formatted = humantime::format_duration(duration);
    serializer.serialize_str(&formatted.to_string())
}

fn default_shell() -> String {
    String::new()
}

fn default_enable_transcript() -> bool {
    true
}

fn default_separate_window() -> bool {
    false
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalAttributes {
    #[serde(default = "default_shell")]
    pub shell: String,
    // We either wait for the terminal to close and are able to capture the output
    // or we don't wait (runs in background) and can't capture the output
    pub wait: bool,
    #[serde(default = "default_separate_window")]
    pub separate_window: bool,
    #[serde(default = "default_enable_transcript")]
    pub enable_transcript: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged, rename_all = "lowercase")]
pub enum ActionAttributes {
    Binary(BinaryAttributes),
    Command(CommandAttributes),
    Store(StoreAttributes),
    Terminal(TerminalAttributes),
    Yara(YaraAttributes),
}

fn replace_in_value(value: Value, variables: &HashMap<String, String>) -> Value {
    match value {
        Value::String(s) => {
            let mut result = s;
            for (key, val) in variables {
                result = result.replace(&format!("${{{}}}", key), val);
            }
            Value::String(result)
        }
        Value::Sequence(seq) => {
            let mut result = Vec::new();
            for item in seq {
                result.push(replace_in_value(item, variables));
            }
            Value::Sequence(result)
        }
        Value::Mapping(map) => {
            let mut result = Mapping::new();
            for (key, val) in map {
                result.insert(key, replace_in_value(val, variables));
            }
            Value::Mapping(result)
        }
        other => other,
    }
}

impl ActionAttributes {
    pub fn replace_vars(&mut self, variables: &HashMap<String, String>) {
        let cloned_self = self.clone();
        let value = serde_yaml::to_value(cloned_self).unwrap();
        let updated_value = replace_in_value(value, variables);
        *self = serde_yaml::from_value(updated_value).unwrap();
    }
}

// implement into so that we can convert ActionAttributes to either BinaryAttributes or CommandAttributes
impl Into<BinaryAttributes> for ActionAttributes {
    fn into(self) -> BinaryAttributes {
        match self {
            ActionAttributes::Binary(binary) => binary,
            _ => panic!("ActionAttributes is not Binary"),
        }
    }
}
impl Into<CommandAttributes> for ActionAttributes {
    fn into(self) -> CommandAttributes {
        match self {
            ActionAttributes::Command(command) => command,
            _ => panic!("ActionAttributes is not Command"),
        }
    }
}
impl Into<StoreAttributes> for ActionAttributes {
    fn into(self) -> StoreAttributes {
        match self {
            ActionAttributes::Store(store) => store,
            _ => panic!("ActionAttributes is not Store"),
        }
    }
}
impl Into<TerminalAttributes> for ActionAttributes {
    fn into(self) -> TerminalAttributes {
        match self {
            ActionAttributes::Terminal(terminal) => terminal,
            _ => panic!("ActionAttributes is not Terminal"),
        }
    }
}
impl Into<YaraAttributes> for ActionAttributes {
    fn into(self) -> YaraAttributes {
        match self {
            ActionAttributes::Yara(yara) => yara,
            _ => panic!("ActionAttributes is not Yara"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    #[serde(deserialize_with = "deserialize_action")]
    pub action_type: ActionType,
    pub attributes: ActionAttributes,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Reporting {
    pub zip_archive: ReportingZipArchive,
    pub metadata: ReportingMetadata,
}
impl Default for Reporting {
    fn default() -> Self {
        Self {
            zip_archive: ReportingZipArchive::default(),
            metadata: ReportingMetadata::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportingZipArchive {
    pub enabled: bool,
    pub encryption: ReportingEncryption,
    pub compression: ReportingCompression,
}
impl Default for ReportingZipArchive {
    fn default() -> Self {
        Self {
            enabled: true,
            encryption: ReportingEncryption::default(),
            compression: ReportingCompression::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum Algorithm {
    #[serde(rename = "AES-128-GCM")]
    // https://datatracker.ietf.org/doc/html/rfc5116
    AES128GCM,
    #[serde(rename = "CHACHA20-POLY1305")]
    // https://datatracker.ietf.org/doc/html/rfc8439
    CHACHA20POLY1305,
    None,
}
impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Algorithm::AES128GCM => write!(f, "AES-128-GCM"),
            Algorithm::CHACHA20POLY1305 => write!(f, "CHACHA20-POLY1305"),
            Algorithm::None => write!(f, "None"),
        }
    }
}
impl Algorithm {
    pub fn block_size(&self) -> usize {
        match self {
            Algorithm::AES128GCM => 4096 * 4,
            Algorithm::CHACHA20POLY1305 => 4096 * 4,
            Algorithm::None => 0,
        }
    }
    pub fn tag_size(&self) -> usize {
        match self {
            Algorithm::AES128GCM => 16,
            Algorithm::CHACHA20POLY1305 => 16,
            Algorithm::None => 0,
        }
    }
    pub fn key_size(&self) -> usize {
        match self {
            Algorithm::AES128GCM => 16,
            Algorithm::CHACHA20POLY1305 => 32,
            Algorithm::None => 0,
        }
    }
    pub fn iv_size(&self) -> usize {
        match self {
            Algorithm::AES128GCM => 12,
            Algorithm::CHACHA20POLY1305 => 12,
            Algorithm::None => 0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportingEncryption {
    pub enabled: bool,
    pub public_key: String,
    pub algorithm: Algorithm,
}
impl Default for ReportingEncryption {
    fn default() -> Self {
        Self {
            enabled: false,
            public_key: "".to_string(),
            algorithm: Algorithm::None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportingCompression {
    pub enabled: bool,
    #[serde(deserialize_with = "deserialize_size_limit")]
    pub size_limit: u64,
}
fn deserialize_size_limit<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;

    match Byte::from_str(&s) {
        Ok(bytes) => Ok(bytes.as_u64()),
        Err(_) => Err(serde::de::Error::custom("Invalid size limit")),
    }
}
fn serialize_size_limit<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let bytes = Byte::from_u64(value.clone());
    serializer.serialize_str(bytes.to_string().as_str())
}

impl Default for ReportingCompression {
    fn default() -> Self {
        Self {
            enabled: false,
            size_limit: 0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportingMetadata {
    pub mac_times: bool,
    pub checksums: bool,
    pub paths: bool,
}
impl Default for ReportingMetadata {
    fn default() -> Self {
        Self {
            mac_times: false,
            checksums: false,
            paths: false,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum OnError {
    #[serde(rename = "goto")]
    Goto { goto: String },
    #[serde(rename = "abort")]
    Abort,
    #[serde(rename = "continue")]
    Continue,
}

impl PartialEq for OnError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OnError::Goto { goto: a }, OnError::Goto { goto: b }) => a == b,
            (OnError::Abort, OnError::Abort) => true,
            (OnError::Continue, OnError::Continue) => true,
            _ => false,
        }
    }
}

fn default_on_error() -> OnError {
    OnError::Continue
}

fn default_parallel() -> bool {
    false
}

fn default_timeout() -> i32 {
    0
}

#[derive(Debug, Deserialize, Clone)]
pub struct WorkflowItem {
    pub action: String,
    #[serde(default = "default_on_error")]
    #[serde(deserialize_with = "deserialize_on_error")]
    pub on_error: OnError,
    #[serde(default = "default_parallel")]
    pub parallel: bool,
    #[serde(default = "default_timeout")]
    #[serde(deserialize_with = "deserialize_timeout")]
    #[serde(serialize_with = "serialize_timeout")]
    pub timeout: i32,
    #[serde(default)]
    pub continue_after_keypress: bool,
}

fn deserialize_on_error<'de, D>(deserializer: D) -> Result<OnError, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;
    if let Some(s) = value.as_str() {
        match s {
            "abort" => return Ok(OnError::Abort),
            "continue" => return Ok(OnError::Continue),
            _ => {}
        }
    } else if let Some(map) = value.as_mapping() {
        if let Some(goto_value) = map.get(&Value::String("goto".to_string())) {
            if let Some(goto_str) = goto_value.as_str() {
                return Ok(OnError::Goto {
                    goto: goto_str.to_string(),
                });
            }
        }
    }
    Err(serde::de::Error::custom("Invalid OnError value"))
}

fn deserialize_action<'de, D>(deserializer: D) -> Result<ActionType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;

    match s.as_str() {
        "binary" => Ok(ActionType::Binary),
        "command" => Ok(ActionType::Command),
        "store" => Ok(ActionType::Store),
        "yara" => Ok(ActionType::Yara),
        "terminal" => Ok(ActionType::Terminal),
        _ => Err(serde::de::Error::custom("Invalid action type")),
    }
}

#[derive(Debug, Deserialize)]
pub struct WorkflowRunner {
    pub properties: HashMap<String, String>,
    pub launch_conditions: LaunchConditions,
    pub actions: Vec<Action>,
    pub workflow: Vec<WorkflowItem>,
    pub reporting: Reporting,
}

impl WorkflowRunner {
    // Check for invalid combinations of settings
    pub fn validate(&mut self, file_name: Option<&str>) -> Result<(), Box<dyn Error>> {
        let mut conflicts: Vec<String> = Vec::new();
        let mut fatal = false;

        // Invalid properties settings
        let required_properties = vec!["title", "version"];
        for prop in required_properties {
            if !self.properties.contains_key(prop) {
                conflicts.push(format!("Properties requires the key: {:?} (fatal)", prop));
                fatal = true;
            }
        }

        // Invalid LaunchConditions settings
        // if custom_command is set, either contains_any, contains_all or contains_regex must be set
        if let Some(custom_command) = &self.launch_conditions.custom_command {
            if custom_command.contains_any.is_none()
                && custom_command.contains_all.is_none()
                && custom_command.contains_regex.is_none()
            {
                conflicts.push("custom_command is set, but neither contains_any, contains_all nor contains_regex is set: disabling custom_command".to_string());
                self.launch_conditions.custom_command = None;
            }
        }

        // Invalid Reporting settings
        // If archive is disabled, encryption and compression cannot be enabled
        if !self.reporting.zip_archive.enabled
            && (self.reporting.zip_archive.encryption.enabled
                || self.reporting.zip_archive.compression.enabled)
        {
            // Add "zip_archive is disabled: encryption and compression will be disabled as well" to the vector
            conflicts.push(
                "zip_archive is disabled: encryption and compression will be disabled as well"
                    .to_string(),
            );
            self.reporting.zip_archive.encryption.enabled = false;
            self.reporting.zip_archive.compression.enabled = false;
        }
        // If archive is disabled, encryption cannot be enabled
        if !self.reporting.zip_archive.encryption.enabled
            && self.reporting.zip_archive.encryption.algorithm != Algorithm::None
        {
            conflicts.push(
                "report can only be encrypted if zip_archive is enable: disabling encryption"
                    .to_string(),
            );
            self.reporting.zip_archive.encryption.algorithm = Algorithm::None;
            self.reporting.zip_archive.encryption.enabled = false;
        }
        // If archive is disabled, compression cannot be enabled
        if !self.reporting.zip_archive.encryption.enabled
            && self.reporting.zip_archive.compression.enabled
        {
            conflicts.push(
                "report can only be compressed if zip_archive is enable: disabling compression"
                    .to_string(),
            );
            self.reporting.zip_archive.compression.enabled = false;
        }

        // Invalid Action settings
        let mut action_names = HashMap::new();
        for action in self.actions.iter_mut() {
            if action.action_type == ActionType::Terminal {
                if let ActionAttributes::Terminal(ref mut terminal) = action.attributes {
                    // Make sure that wait and separate_window are not both false at the same time
                    if !terminal.wait && !terminal.separate_window {
                        conflicts.push(format!("Action {:?} has both wait and separate_window set to false: setting wait to true", action.name));
                        terminal.wait = true;
                    }

                    // Make sure that the transcript is not enabled while not waiting for the session to finish
                    // Allowed:
                    // - wait: true, enable_transcript: true
                    // - wait: true, enable_transcript: false
                    // - wait: false, enable_transcript: false
                    // If we don't wait, we can't save the transcript output in the archive
                    // The downside is that we delay the execution of the next action and encryption of the archive
                    // It's not like we can't do it, but it introduces a lot of complexity
                    if !terminal.wait && terminal.enable_transcript {
                        conflicts.push(format!("Action {:?} has enable_transcript set to true while not waiting for the terminal to close. Disabling transcript...", action.name));
                        terminal.enable_transcript = false;
                    }
                }
            }

            // Check for duplicate action names
            if action_names.contains_key(&action.name) {
                conflicts.push(format!("Duplicate action name: {:?} (fatal)", action.name));
                fatal = true;
            } else {
                action_names.insert(action.name.clone(), ());
            }
        }

        // Invalid Workflow settings
        for item in self.workflow.iter_mut() {
            // If parallel is enabled we can't wait for a keypress
            if item.parallel && item.continue_after_keypress {
                conflicts.push(format!("Action {:?} is set to run in parallel and wait for keypress at the same time. Disabling continue_after_keypress...", item.action));
                item.continue_after_keypress = false;
            }

            for action in self.actions.iter_mut() {
                if action.name == item.action {
                    // If an action is set to run in parallel, it must be one of the allowed action types
                    if item.parallel && !parallel_action_types().contains(&action.action_type) {
                        conflicts.push(format!("Action {:?} is set to run in parallel, but it is not allowed to run in parallel. Disabling parallel execution...", action.name));
                        item.parallel = false;
                    }

                    // If an action has a timeout, it must be one of the allowed action types
                    if item.timeout > 0 && !timeout_action_types().contains(&action.action_type) {
                        conflicts.push(format!("Action {:?} has a timeout set, but it is not allowed to have a timeout. Disabling timeout...", action.name));
                        item.timeout = 0;
                    }

                    // If parallel is enabled we need to log into a file, so we can easily capture the output
                    if item.parallel {
                        match action.attributes {
                            ActionAttributes::Binary(ref mut ba) => {
                                if !ba.log_to_file {
                                    conflicts.push(format!("Action {:?} is set to run in parallel, but log_to_file is disabled. Setting log_to_file to true...", action.name));
                                    ba.log_to_file = true;
                                }
                            }
                            ActionAttributes::Command(ref mut ca) => {
                                if !ca.log_to_file {
                                    conflicts.push(format!("Action {:?} is set to run in parallel and log_to_file is disabled. Setting log_to_file to true...", action.name));
                                    ca.log_to_file = true;
                                }
                            }
                            ActionAttributes::Terminal(ref mut ta) => {
                                if !ta.separate_window {
                                    conflicts.push(format!("Action {:?} is set to run in parallel but uses integrated terminal at the same time. Setting parallel to false...", action.name));
                                    item.parallel = false;
                                }
                            }
                            _ => {}
                        }
                    }

                    // Parallel and custom on_error are not compatible
                    if item.parallel && item.on_error != OnError::Continue {
                        conflicts.push(format!("Action {:?} is set to run in parallel and has a custom on_error. Setting on_error to continue...", action.name));
                        item.on_error = OnError::Continue;
                    }
                }
            }
        }

        // Generate warnings for each conflict
        if conflicts.is_empty() {
            return Ok(());
        }

        let mut message = String::new();
        message.push_str(
            format!(
                "Found conflict in workflow {:?}:\n",
                file_name.unwrap_or("N/A")
            )
            .as_str(),
        );
        for conflict in conflicts {
            message.push_str(&format!("\t- {}\n", conflict));
        }
        // delete the last newline
        message.pop();

        if fatal {
            error!("{}", message);
            return Err("Fatal conflicts found in workflow".into());
        } else {
            warn!("{}", message);
        }

        Ok(())
    }
}

pub fn read_workflow_file(yaml_path: &PathBuf) -> Result<WorkflowRunner, Box<dyn Error>> {
    let file = File::open(yaml_path)?;
    let reader = BufReader::new(file);
    let mut runner: WorkflowRunner = match serde_yaml::from_reader(reader) {
        Ok(runner) => runner,
        Err(e) => {
            error!("Error parsing workflow schema: {}", e);
            return Err(Box::new(e));
        }
    };

    let file_name = std::path::Path::new(yaml_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    match runner.validate(Some(file_name)) {
        Ok(_) => {}
        Err(e) => {
            return Err(e);
        }
    }

    Ok(runner)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use system::SystemVariables;
    use utils::tests::Cleanup;

    #[test]
    fn test_deserialize_launch_conditions_valid() {
        let yaml = r#"
            os: ["windows", "linux", "macos"]
            enabled: true
            arch: ["x86", "x86_64", "aarch64", "arm"]
            is_elevated: false
            custom_command:
                cmd: "cmd"
                args: ["/c", "dir", "${USER_HOME}"]
                contains_any: ["Hello World"]
                contains_all: ["Hello", "World"]
                contains_regex: "Hello.*World"
        "#;
        let mut lc: LaunchConditions = serde_yaml::from_str(yaml).unwrap();

        let variables = SystemVariables::new();
        lc.custom_command
            .as_mut()
            .unwrap()
            .replace_vars(&variables.as_map());

        let binding = variables.user_home.to_string_lossy();
        let user_home: &str = binding.as_ref();

        assert_eq!(lc.os, vec!["windows", "linux", "macos"]);
        assert_eq!(lc.enabled.unwrap(), true);
        assert_eq!(lc.arch.unwrap(), vec!["x86", "x86_64", "aarch64", "arm"]);
        assert_eq!(lc.is_elevated.unwrap(), false);
        assert_eq!(lc.custom_command.as_ref().unwrap().cmd, "cmd");
        assert_eq!(
            lc.custom_command.as_ref().unwrap().args.clone().unwrap(),
            vec!["/c", "dir", user_home]
        );
        assert_eq!(
            lc.custom_command
                .as_ref()
                .unwrap()
                .contains_any
                .clone()
                .unwrap(),
            vec!["Hello World".to_string()]
        );
        assert_eq!(
            lc.custom_command
                .as_ref()
                .unwrap()
                .contains_all
                .clone()
                .unwrap(),
            vec!["Hello".to_string(), "World".to_string()]
        );
        assert_eq!(
            lc.custom_command
                .as_ref()
                .unwrap()
                .contains_regex
                .clone()
                .unwrap(),
            "Hello.*World"
        );
    }

    #[test]
    fn test_deserialize_binary_attributes() {
        let yaml = r#"
            path: "/usr/bin/test"
            args: ["--verbose"]
            log_to_file: true
        "#;
        let ba: BinaryAttributes = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ba.path, "/usr/bin/test");
        assert_eq!(ba.args, vec!["--verbose"]);
        assert!(ba.log_to_file);
    }

    #[test]
    fn test_deserialize_command_attributes() {
        let yaml = r#"
            cmd: "echo"
            args: ["Hello, world!"]
            log_to_file: false
        "#;
        let ca: CommandAttributes = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ca.cmd, "echo");
        assert_eq!(ca.args, vec!["Hello, world!"]);
        assert!(!ca.log_to_file);
    }

    #[test]
    fn test_deserialize_action_attributes() {
        let yaml_binary = r#"
            path: "/usr/bin/test"
            args: ["--verbose"]
            log_to_file: true
        "#;
        let ba: BinaryAttributes = serde_yaml::from_str(yaml_binary).unwrap();
        let aa: ActionAttributes = ActionAttributes::Binary(ba.clone());
        let converted_ba: BinaryAttributes = aa.into();
        assert_eq!(converted_ba.path, "/usr/bin/test");
        assert_eq!(converted_ba.args, vec!["--verbose"]);
        assert!(converted_ba.log_to_file);

        let yaml_command = r#"
            cmd: "echo"
            args: ["Hello, world!"]
            log_to_file: false
        "#;
        let ca: CommandAttributes = serde_yaml::from_str(yaml_command).unwrap();
        let aa: ActionAttributes = ActionAttributes::Command(ca.clone());
        let converted_ca: CommandAttributes = aa.into();
        assert_eq!(converted_ca.cmd, "echo");
        assert_eq!(converted_ca.args, vec!["Hello, world!"]);
        assert!(!converted_ca.log_to_file);
    }

    #[test]
    fn test_deserialize_reporting() {
        let yaml = r#"
        zip_archive:
            enabled: true
            encryption:
                enabled: true
                public_key: "some_key"
                algorithm: "AES-128-GCM"
            compression:
                enabled: true
                size_limit: "10 MB"
        metadata:
            mac_times: true
            checksums: true
            paths: true
        "#;
        let reporting: Reporting = serde_yaml::from_str(yaml).unwrap();
        assert!(reporting.zip_archive.enabled);
        assert!(reporting.zip_archive.encryption.enabled);
        assert_eq!(reporting.zip_archive.encryption.public_key, "some_key");
        assert_eq!(
            reporting.zip_archive.encryption.algorithm,
            Algorithm::AES128GCM
        );
        assert!(reporting.zip_archive.compression.enabled);
        assert_eq!(reporting.zip_archive.compression.size_limit, 10_000_000);
        assert!(reporting.metadata.mac_times);
        assert!(reporting.metadata.checksums);
        assert!(reporting.metadata.paths);
    }

    #[test]
    fn test_read_workflow_file() {
        let yaml_content = r#"
        properties:
          title: "value1"
          version: "value2"
        launch_conditions:
          os: ["linux"]
          arch: ["x86_64"]
        actions:
          - name: "Test Action"
            type: "binary"
            attributes:
              path: "/bin/true"
              args: []
              log_to_file: false
        workflow:
          - action: "Test Action"
        reporting:
          zip_archive:
            enabled: true
            encryption:
              enabled: false
              public_key: ""
              algorithm: None
            compression:
              enabled: false
              size_limit: "0"
          metadata:
            mac_times: false
            checksums: false
            paths: false
        "#;
        let mut cleanup = Cleanup::new();
        // create a directory to store the workflow file
        let dir = cleanup.tmp_dir("test_read_workflow_file");

        let file_path = dir.join("workflow.yaml");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(yaml_content.as_bytes()).unwrap();

        let workflow = read_workflow_file(&file_path).unwrap();
        assert_eq!(workflow.properties["title"], "value1");
        assert_eq!(workflow.properties["version"], "value2");
        assert_eq!(workflow.launch_conditions.os, vec!["linux"]);
        assert_eq!(workflow.launch_conditions.arch.unwrap(), vec!["x86_64"]);
        assert_eq!(workflow.actions.len(), 1);
        assert_eq!(workflow.actions[0].name, "Test Action");
        assert_eq!(workflow.actions[0].action_type, ActionType::Binary);
        if let ActionAttributes::Binary(ref ba) = workflow.actions[0].attributes {
            assert_eq!(ba.path, "/bin/true");
            assert!(!ba.log_to_file);
        } else {
            panic!("Expected ActionAttributes::Binary variant");
        }
        assert_eq!(workflow.workflow.len(), 1);
        assert_eq!(workflow.workflow[0].action, "Test Action");
        assert_eq!(workflow.workflow[0].on_error, OnError::Continue);
    }

    #[test]
    fn test_deserialize_on_error() {
        let yaml = r#"
          - action: test1
            on_error: continue
            parallel: false
          - action: test2
            on_error: abort
            parallel: false
          - action: test3
            on_error:
              goto: test
            parallel: false
        "#;

        let workflow: Vec<WorkflowItem> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow[0].on_error, OnError::Continue);
        assert_eq!(workflow[1].on_error, OnError::Abort);
        assert_eq!(
            workflow[2].on_error,
            OnError::Goto {
                goto: "test".to_string()
            }
        );
    }
}
