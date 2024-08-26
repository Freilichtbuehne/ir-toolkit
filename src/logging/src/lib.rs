use config::config::Time;
use system::get_base_path;
use time::get_ntp_time;

use chrono::{Local, Utc};
use chrono_tz::{self, Tz, UTC};
use fern::colors::{Color, ColoredLevelConfig};
use log::{error, info, warn};
use log::{Level, LevelFilter};
use std::{fs, panic};

pub struct Logger {
    _status: Option<String>,
    file_path: Option<String>,
    start_time: chrono::DateTime<Local>,
    duration: std::time::Instant,
    level: LevelFilter,
    file_level: LevelFilter,
    time_config: Option<Time>,
    time_zone: Tz,
}

fn format_duration(duration: std::time::Duration) -> String {
    let hours = duration.as_secs() / 3600;
    let minutes = (duration.as_secs() % 3600) / 60;
    let seconds = duration.as_secs() % 60;
    let milliseconds = duration.subsec_millis();

    format!(
        "{:02}h:{:02}m:{:02}s:{:03}ms",
        hours, minutes, seconds, milliseconds
    )
}

impl Logger {
    pub fn init() -> Logger {
        let logger = Logger {
            _status: None,
            file_path: None,
            start_time: Local::now(),
            duration: std::time::Instant::now(),
            level: LevelFilter::Info,
            file_level: LevelFilter::Debug,
            time_config: None,
            time_zone: UTC,
        };

        // Create a panic hook
        panic::set_hook(Box::new(|panic_info| {
            let location = panic_info.location().unwrap();
            let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                *s
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic message"
            };
            error!(
                "PANIC at {}:{}: {}",
                location.file(),
                location.line(),
                message
            );
        }));

        logger
    }

    pub fn log_initial_info(&self) {
        let utc_time = Utc::now();

        let ntp_time = match &self.time_config {
            Some(time_config) if time_config.ntp_enabled => get_ntp_time(time_config.clone()),
            _ => None,
        };

        let local_time = Local::now().with_timezone(&self.time_zone);

        let pid = std::process::id();
        let cwd = std::env::current_dir().unwrap();

        let initial_info = format!(
            "\nCWD: {:?}\nPID: {}\nLocal time: {}\nUTC time: {}\nNTP UTC time: {}\nTimezone: {}\n",
            cwd,
            pid,
            local_time.to_rfc3339(),
            utc_time.to_rfc3339(),
            if let Some(ntp_time) = ntp_time {
                ntp_time.to_rfc3339()
            } else {
                "N/A".to_string()
            },
            self.time_zone
        );

        info!("{}", initial_info);
    }

    pub fn apply(self) -> Self {
        let colors = ColoredLevelConfig::new()
            .debug(Color::Blue)
            .info(Color::Green)
            .warn(Color::Yellow)
            .error(Color::Red);

        let mut base_config = fern::Dispatch::new().chain(
            fern::Dispatch::new()
                .level(self.level)
                .format(move |out, message, record| {
                    let time = Local::now()
                        .with_timezone(&self.time_zone)
                        .format("%Y-%m-%d %H:%M:%S");
                    if record.level() == Level::Error {
                        out.finish(format_args!(
                            "[{}] [{}] [{}:{}] {}",
                            time,
                            colors.color(record.level()),
                            record.target(),
                            record.line().unwrap_or(0), // Using 0 as default if line is None
                            message
                        ))
                    } else {
                        out.finish(format_args!(
                            "[{}] [{}] [{}] {}",
                            time,
                            colors.color(record.level()),
                            record.target(),
                            message
                        ))
                    }
                })
                .chain(std::io::stdout()),
        );

        if let Some(ref file_path) = self.file_path {
            base_config = base_config.chain(
                fern::Dispatch::new()
                    .format(move |out, message, record| {
                        let time = Local::now().with_timezone(&self.time_zone).to_rfc3339();
                        if record.level() == Level::Error {
                            out.finish(format_args!(
                                "[{}] [{}] [{}:{}] {}",
                                time,
                                record.level(),
                                record.target(),
                                record.line().unwrap_or(0),
                                message
                            ))
                        } else {
                            out.finish(format_args!(
                                "[{}] [{}] [{}] {}",
                                time,
                                record.level(),
                                record.target(),
                                message
                            ))
                        }
                    })
                    .level(self.file_level)
                    .chain(fern::log_file(file_path).unwrap()),
            );
        }

        base_config.apply().unwrap();

        self
    }

    pub fn set_file(mut self) -> Self {
        // check if reports directory exists and create it if not
        let reports_dir = get_base_path().join("reports");
        if !reports_dir.exists() {
            fs::create_dir(&reports_dir).expect("Failed to create reports directory");
        }

        // create log file
        let file_name = format!("{}.log", self.start_time.format("%Y-%m-%d_%H-%M-%S"));
        let log_file = reports_dir.join(&file_name);
        self.file_path = Some(log_file.to_str().unwrap().to_string());

        self
    }

    #[cfg(test)]
    pub fn get_file(&self) -> Option<String> {
        self.file_path.clone()
    }

    pub fn set_level(mut self, level: LevelFilter) -> Self {
        self.level = level;
        self
    }

    pub fn set_file_level(mut self, level: LevelFilter) -> Self {
        self.file_level = level;
        self
    }

    pub fn set_time_config(mut self, config: Time) -> Self {
        // set timezone
        let time_zone = config.time_zone.clone();
        self.time_zone = match time_zone.parse() {
            Ok(tz) => tz,
            Err(_) => {
                warn!("Invalid timezone: {:?}. Using UTC instead.", time_zone);
                UTC
            }
        };

        // set time config
        self.time_config = Some(config);
        self
    }

    pub fn finish(&self) {
        Local::now();
        let duration = self.duration.elapsed();
        let summary = format!(
            "Collection finished. Duration: {}\n",
            format_duration(duration)
        );

        info!("{}", summary);

        // flush the logger
        log::logger().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::config::Time;
    use log::{debug, error, info, warn};
    use std::fs;
    use std::path::PathBuf;
    use utils::tests::Cleanup;

    #[test]
    fn test_logger_init() {
        let logger = Logger::init();
        assert_eq!(logger._status, None);
        assert_eq!(logger.file_path, None);
        assert_eq!(logger.level, LevelFilter::Info);
        assert_eq!(logger.time_zone, UTC);
    }

    #[test]
    fn test_logger_set_file() {
        let mut cleanup = Cleanup::new();

        let logger = Logger::init().set_file().apply();

        let log_file = logger.get_file().unwrap();
        let log_file = PathBuf::from(&log_file);
        assert!(log_file.exists());
        cleanup.add(log_file.clone());

        debug!("Test log message");
        info!("Test log message");
        warn!("Test log message");
        error!("Test log message");

        let log_content = fs::read_to_string(&log_file).unwrap();
        assert!(log_content.contains("Test log message"));
    }

    #[test]
    fn test_logger_set_level() {
        let mut cleanup = Cleanup::new();

        let logger = Logger::init()
            .set_file()
            .set_level(LevelFilter::Warn)
            .set_file_level(LevelFilter::Warn)
            .apply();

        let log_file = logger.get_file().unwrap();
        let log_file = PathBuf::from(&log_file);
        assert!(log_file.exists());
        cleanup.add(log_file.clone());

        debug!("Don't log this message");
        info!("Don't log this message");
        warn!("Log this message");
        error!("Log this message");

        let log_content = fs::read_to_string(&log_file).unwrap();
        assert!(!log_content.contains("Don't log this message"));
        assert!(log_content.contains("Log this message"));
    }

    #[test]
    fn test_logger_set_time_config() {
        let time_config = Time {
            time_zone: "America/New_York".to_string(),
            ntp_enabled: false,
            ntp_servers: vec![],
            ntp_timeout: 0,
        };

        let logger = Logger::init().set_time_config(time_config.clone());
        assert_eq!(logger.time_zone, chrono_tz::America::New_York);
    }

    #[test]
    fn test_panic_hook() {
        // cause a panic and check if it appears in the log
        let mut cleanup = Cleanup::new();

        let logger = Logger::init().set_file().apply();

        let log_file = logger.get_file().unwrap();
        let log_file = PathBuf::from(&log_file);
        assert!(log_file.exists());
        cleanup.add(log_file.clone());

        // catch the panic message
        let _ = panic::catch_unwind(|| {
            panic!("This is a panic message");
        });

        let log_content = fs::read_to_string(&log_file).unwrap();
        assert!(log_content.contains("This is a panic message"));
    }
}
