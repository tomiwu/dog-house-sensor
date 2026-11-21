//! The 12-byte radio payload (PRD section 6). All fields are little-endian.

/// Version of the packet format.
pub const FORMAT_VERSION: u8 = 1;
/// Length of the payload in bytes.
pub const PACKET_LEN: usize = 12;

/// Status bit 0: the sensor read failed.
pub const STATUS_SENSOR_ERROR: u8 = 1 << 0;
/// Status bit 1: the sensor checksum was wrong.
pub const STATUS_CHECKSUM_ERROR: u8 = 1 << 1;
/// Status bit 2: this is the first boot.
pub const STATUS_FIRST_BOOT: u8 = 1 << 2;

const TEMP_DC_RANGE: (i16, i16) = (-400, 800);
const HUM_DPCT_MAX: u16 = 1000;
const VBAT_MV_RANGE: (u16, u16) = (2000, 4500);
const PREV_RETRIES_MAX: u8 = 15;

/// One sensor reading. The version is not a field: [`encode`] always writes
/// [`FORMAT_VERSION`] and [`decode`] rejects any other version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    /// Node number. 1 is the dog house.
    pub node_id: u8,
    /// Counter that grows on each wake.
    pub seq: u16,
    /// Temperature in 0.1 °C. Range -400 to 800.
    pub temp_dc: i16,
    /// Relative humidity in 0.1 %. Range 0 to 1000.
    pub hum_dpct: u16,
    /// Battery voltage in mV. Range 2000 to 4500.
    pub vbat_mv: u16,
    /// Status bits. See the `STATUS_*` constants.
    pub status: u8,
    /// Auto-retransmit count of the previous packet. Range 0 to 15.
    pub prev_retries: u8,
}

/// The field that has a value out of range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    TempDc,
    HumDpct,
    VbatMv,
    PrevRetries,
}

/// Why a packet is not valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketError {
    /// The input is not [`PACKET_LEN`] bytes long.
    WrongLength { actual: usize },
    /// The first byte is not [`FORMAT_VERSION`].
    UnknownVersion(u8),
    /// A value is outside the allowed range.
    OutOfRange(Field),
}

fn in_range<T: PartialOrd>(value: T, (min, max): (T, T), field: Field) -> Result<(), PacketError> {
    if value < min || value > max {
        Err(PacketError::OutOfRange(field))
    } else {
        Ok(())
    }
}

impl Packet {
    /// Check the range of each value.
    pub fn validate(&self) -> Result<(), PacketError> {
        in_range(self.temp_dc, TEMP_DC_RANGE, Field::TempDc)?;
        in_range(self.hum_dpct, (0, HUM_DPCT_MAX), Field::HumDpct)?;
        in_range(self.vbat_mv, VBAT_MV_RANGE, Field::VbatMv)?;
        in_range(self.prev_retries, (0, PREV_RETRIES_MAX), Field::PrevRetries)
    }
}

/// Write a packet as 12 bytes. Fails if a value is out of range.
pub fn encode(packet: &Packet) -> Result<[u8; PACKET_LEN], PacketError> {
    packet.validate()?;
    let mut out = [0u8; PACKET_LEN];
    out[0] = FORMAT_VERSION;
    out[1] = packet.node_id;
    out[2..4].copy_from_slice(&packet.seq.to_le_bytes());
    out[4..6].copy_from_slice(&packet.temp_dc.to_le_bytes());
    out[6..8].copy_from_slice(&packet.hum_dpct.to_le_bytes());
    out[8..10].copy_from_slice(&packet.vbat_mv.to_le_bytes());
    out[10] = packet.status;
    out[11] = packet.prev_retries;
    Ok(out)
}

/// Read a packet from bytes. Checks length, version and ranges.
pub fn decode(bytes: &[u8]) -> Result<Packet, PacketError> {
    let raw: &[u8; PACKET_LEN] = bytes.try_into().map_err(|_| PacketError::WrongLength {
        actual: bytes.len(),
    })?;
    if raw[0] != FORMAT_VERSION {
        return Err(PacketError::UnknownVersion(raw[0]));
    }
    let packet = Packet {
        node_id: raw[1],
        seq: u16::from_le_bytes([raw[2], raw[3]]),
        temp_dc: i16::from_le_bytes([raw[4], raw[5]]),
        hum_dpct: u16::from_le_bytes([raw[6], raw[7]]),
        vbat_mv: u16::from_le_bytes([raw[8], raw[9]]),
        status: raw[10],
        prev_retries: raw[11],
    };
    packet.validate()?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Packet {
        Packet {
            node_id: 1,
            seq: 258,
            temp_dc: 215,
            hum_dpct: 553,
            vbat_mv: 3300,
            status: STATUS_FIRST_BOOT,
            prev_retries: 2,
        }
    }

    #[test]
    fn format_version_starts_at_one() {
        assert_eq!(FORMAT_VERSION, 1);
    }

    #[test]
    fn round_trip_keeps_all_fields() {
        let bytes = encode(&sample()).unwrap();
        assert_eq!(decode(&bytes), Ok(sample()));
    }

    #[test]
    fn round_trip_at_range_limits() {
        for (temp_dc, hum_dpct, vbat_mv, prev_retries) in
            [(-400, 0, 2000, 0), (800, 1000, 4500, 15)]
        {
            let packet = Packet {
                temp_dc,
                hum_dpct,
                vbat_mv,
                prev_retries,
                ..sample()
            };
            assert_eq!(decode(&encode(&packet).unwrap()), Ok(packet));
        }
    }

    #[test]
    fn encode_uses_little_endian_and_version() {
        let bytes = encode(&sample()).unwrap();
        assert_eq!(bytes, [1, 1, 2, 1, 215, 0, 41, 2, 228, 12, 4, 2]);
    }

    #[test]
    fn decode_rejects_wrong_length() {
        assert_eq!(decode(&[]), Err(PacketError::WrongLength { actual: 0 }));
        let bytes = encode(&sample()).unwrap();
        assert_eq!(
            decode(&bytes[..11]),
            Err(PacketError::WrongLength { actual: 11 })
        );
        let mut long = [0u8; 13];
        long[..12].copy_from_slice(&bytes);
        assert_eq!(decode(&long), Err(PacketError::WrongLength { actual: 13 }));
    }

    #[test]
    fn decode_rejects_unknown_version() {
        let mut bytes = encode(&sample()).unwrap();
        bytes[0] = 2;
        assert_eq!(decode(&bytes), Err(PacketError::UnknownVersion(2)));
    }

    #[test]
    fn decode_rejects_each_value_out_of_range() {
        let cases = [
            (
                Packet {
                    temp_dc: 801,
                    ..sample()
                },
                Field::TempDc,
            ),
            (
                Packet {
                    temp_dc: -401,
                    ..sample()
                },
                Field::TempDc,
            ),
            (
                Packet {
                    hum_dpct: 1001,
                    ..sample()
                },
                Field::HumDpct,
            ),
            (
                Packet {
                    vbat_mv: 1999,
                    ..sample()
                },
                Field::VbatMv,
            ),
            (
                Packet {
                    vbat_mv: 4501,
                    ..sample()
                },
                Field::VbatMv,
            ),
            (
                Packet {
                    prev_retries: 16,
                    ..sample()
                },
                Field::PrevRetries,
            ),
        ];
        for (packet, field) in cases {
            assert_eq!(encode(&packet), Err(PacketError::OutOfRange(field)));
        }
    }

    #[test]
    fn decode_checks_ranges_in_the_bytes() {
        let mut bytes = encode(&sample()).unwrap();
        bytes[4..6].copy_from_slice(&i16::MIN.to_le_bytes());
        assert_eq!(decode(&bytes), Err(PacketError::OutOfRange(Field::TempDc)));
    }
}
