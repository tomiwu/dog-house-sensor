//! Raspberry Pi bridge for the dog house sensor.
//!
//! Issues #4, #5 and #6 add the configuration, the radio and MQTT.

fn banner() -> String {
    format!(
        "dog-house-bridge {} (packet format {})",
        env!("CARGO_PKG_VERSION"),
        protocol::FORMAT_VERSION
    )
}

fn main() {
    println!("{}", banner());
}

#[cfg(test)]
mod tests {
    use super::banner;

    #[test]
    fn banner_names_the_packet_format() {
        assert!(banner().ends_with("(packet format 1)"));
    }
}
