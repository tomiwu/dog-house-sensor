//! Raspberry Pi bridge for the dog house sensor.
//!
//! Issues #5 and #6 add the radio and MQTT.
#![forbid(unsafe_code)]

mod config;
mod research;

use std::path::PathBuf;
use std::process::ExitCode;

use config::Config;

const DEFAULT_CONFIG_PATH: &str = "/etc/dog-house-bridge/config.toml";

fn banner() -> String {
    format!(
        "dog-house-bridge {} (packet format {})",
        env!("CARGO_PKG_VERSION"),
        protocol::FORMAT_VERSION
    )
}

fn main() -> ExitCode {
    println!("{}", banner());
    let path = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH), PathBuf::from);
    match Config::load(&path) {
        Ok(config) => {
            if let Some(warning) = config.radio.warning() {
                research::spawn_hourly_warning(warning);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::banner;

    #[test]
    fn banner_names_the_packet_format() {
        assert!(banner().ends_with("(packet format 1)"));
    }
}
