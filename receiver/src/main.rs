//! Raspberry Pi bridge for the dog house sensor.
//!
//! The bridge connects to MQTT and publishes the readings. Issue #5 adds the radio.
#![forbid(unsafe_code)]

mod bridge;
mod broker;
mod config;
mod payload;
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
            let (readings_tx, readings_rx) = std::sync::mpsc::channel();
            broker::run(config, readings_rx);
            drop(readings_tx);
            ExitCode::FAILURE
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
