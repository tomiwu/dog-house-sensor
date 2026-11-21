//! Radio packet format of the dog house sensor.
//!
//! The node and the bridge share this crate. It has two parts:
//! - [`packet`]: the 12-byte payload with `encode` and `decode` (PRD section 6).
//! - [`radio`]: the legal check for channel and e.i.r.p. (PRD section 7.1a).
#![no_std]

pub mod packet;
pub mod radio;

pub use packet::{
    FORMAT_VERSION, Field, PACKET_LEN, Packet, PacketError, STATUS_CHECKSUM_ERROR,
    STATUS_FIRST_BOOT, STATUS_SENSOR_ERROR, decode, encode,
};
pub use radio::{
    MAX_EIRP_DBM, MAX_LEGAL_CHANNEL, RadioCheck, RadioSettings, Violation, Violations, check_radio,
};
