use log::error;
use serde::Deserialize;
use std::{error::Error, fs::File, io::BufReader, path::PathBuf};

pub const CONFIG_PATH: &str = "config.yaml";

#[derive(Debug, Deserialize, Clone)]
pub struct Time {
    pub time_zone: String,
    pub ntp_enabled: bool,
    pub ntp_servers: Vec<String>,
    pub ntp_timeout: u64,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub time: Time,
    pub elevate: bool,
}

pub fn read_config_file(yaml_path: &PathBuf) -> Result<Config, Box<dyn Error>> {
    let file = File::open(yaml_path)?;
    let reader = BufReader::new(file);
    match serde_yaml::from_reader(reader) {
        Ok(schema) => Ok(schema),
        Err(e) => {
            error!("Error parsing config schema: {}", e);
            Err(Box::new(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use utils::tests::Cleanup;

    fn assert_config_valid(config: &Config) {
        assert_eq!(config.time.time_zone, "UTC");
        assert_eq!(config.time.ntp_enabled, true);
        assert_eq!(
            config.time.ntp_servers,
            vec!["0.pool.ntp.org", "1.pool.ntp.org"]
        );
        assert_eq!(config.time.ntp_timeout, 10);
        assert_eq!(config.elevate, true);
    }

    #[test]
    fn test_read_config_file() {
        let mut cleanup = Cleanup::new();
        let yaml_path = cleanup.tmp_dir("config.yaml").join("config.yaml");

        let yaml_content = r#"
            time:
                time_zone: "UTC"
                ntp_enabled: true
                ntp_servers:
                    - "0.pool.ntp.org"
                    - "1.pool.ntp.org"
                ntp_timeout: 10
            elevate: true
        "#;
        fs::write(&yaml_path, yaml_content).expect("Failed to write config file");

        let config = read_config_file(&yaml_path).unwrap();
        assert_config_valid(&config);
    }
}
