//! Radio packet format of the dog house sensor.
//!
//! The node and the bridge share this crate. Issue #2 adds the packet format.
#![no_std]

/// Version of the packet format (PRD section 6).
pub const FORMAT_VERSION: u8 = 1;

#[cfg(test)]
mod tests {
    use super::FORMAT_VERSION;

    #[test]
    fn format_version_starts_at_one() {
        assert_eq!(FORMAT_VERSION, 1);
    }
}
