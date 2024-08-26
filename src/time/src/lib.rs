use chrono::{DateTime, TimeZone, Utc};
use config::config::Time;
use log::{debug, error};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub fn get_ntp_time(time_config: Time) -> Option<DateTime<Utc>> {
    let (tx, rx) = mpsc::channel();
    let servers = time_config.ntp_servers;
    let timeout_secs = Duration::from_secs(time_config.ntp_timeout);

    thread::spawn(move || {
        for server in servers {
            debug!("Requesting NTP time from server: {}", server);
            let server_start = Instant::now();

            while server_start.elapsed() < timeout_secs {
                match request_ntp_time(&server) {
                    Ok(ntp_time) => {
                        tx.send(Some(ntp_time)).unwrap();
                        return;
                    }
                    Err(e) => {
                        error!("Error contacting NTP server {}: {}", server, e);
                    }
                }
                // Short sleep to avoid busy waiting
                thread::sleep(Duration::from_millis(100));
            }

            error!("NTP request to server {} timed out", server);
        }
        tx.send(None).unwrap();
    });

    // Main thread waits for a response
    match rx.recv() {
        Ok(ntp_time) => ntp_time,
        Err(_) => {
            error!("Failed to receive NTP time");
            None
        }
    }
}

fn request_ntp_time(server: &str) -> Result<DateTime<Utc>, String> {
    match ntp::request(server) {
        Ok(response) => {
            let ntp_time = response.transmit_time;
            let mut unix_time = ntp_time.sec as i64 - 2_208_988_800; // 70 years in seconds

            // Normalize the frac value to be within the valid range
            let mut frac = ntp_time.frac as i64;
            if frac >= 1_000_000_000 {
                let extra_seconds = frac / 1_000_000_000;
                frac = frac % 1_000_000_000;
                unix_time += extra_seconds;
            }

            let ntp_time = Utc.timestamp_opt(unix_time, frac as u32).single();
            if let Some(ntp_time) = ntp_time {
                Ok(ntp_time)
            } else {
                Err("Failed to convert NTP time to DateTime<Utc>".to_string())
            }
        }
        Err(e) => Err(format!("Error: {}", e)),
    }
}
