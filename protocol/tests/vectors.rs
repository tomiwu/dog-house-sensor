//! Runs the golden vectors in `vectors.json` against `encode` and `decode`.

use protocol::{Field, Packet, PacketError, decode, encode};
use serde_json::Value;

fn vectors(group: &str) -> Vec<Value> {
    let all: Value =
        serde_json::from_str(include_str!("vectors.json")).expect("vectors.json is valid JSON");
    all[group].as_array().expect("group is an array").clone()
}

fn bytes_of(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .expect("bytes is an array")
        .iter()
        .map(|b| u8::try_from(b.as_u64().expect("byte is a number")).expect("byte fits u8"))
        .collect()
}

fn int(value: &Value, key: &str) -> i64 {
    value[key]
        .as_i64()
        .unwrap_or_else(|| panic!("{key} missing"))
}

fn packet_of(value: &Value) -> Packet {
    Packet {
        node_id: int(value, "node_id").try_into().unwrap(),
        seq: int(value, "seq").try_into().unwrap(),
        temp_dc: int(value, "temp_dc").try_into().unwrap(),
        hum_dpct: int(value, "hum_dpct").try_into().unwrap(),
        vbat_mv: int(value, "vbat_mv").try_into().unwrap(),
        status: int(value, "status").try_into().unwrap(),
        prev_retries: int(value, "prev_retries").try_into().unwrap(),
    }
}

fn field_of(name: &str) -> Field {
    match name {
        "temp_dc" => Field::TempDc,
        "hum_dpct" => Field::HumDpct,
        "vbat_mv" => Field::VbatMv,
        "prev_retries" => Field::PrevRetries,
        other => panic!("unknown field {other}"),
    }
}

fn error_of(value: &Value) -> PacketError {
    match value["kind"].as_str().expect("kind is a string") {
        "wrong_length" => PacketError::WrongLength {
            actual: int(value, "actual").try_into().unwrap(),
        },
        "unknown_version" => PacketError::UnknownVersion(int(value, "version").try_into().unwrap()),
        "out_of_range" => PacketError::OutOfRange(field_of(value["field"].as_str().unwrap())),
        other => panic!("unknown error kind {other}"),
    }
}

#[test]
fn valid_vectors_decode_to_the_packet() {
    for vector in vectors("valid") {
        let name = &vector["name"];
        let decoded = decode(&bytes_of(&vector["bytes"]));
        assert_eq!(decoded, Ok(packet_of(&vector["packet"])), "{name}");
    }
}

#[test]
fn valid_vectors_encode_to_the_bytes() {
    for vector in vectors("valid") {
        let name = &vector["name"];
        let encoded = encode(&packet_of(&vector["packet"]));
        assert_eq!(
            encoded.map(|b| b.to_vec()),
            Ok(bytes_of(&vector["bytes"])),
            "{name}"
        );
    }
}

#[test]
fn invalid_vectors_fail_with_the_error() {
    for vector in vectors("invalid") {
        let name = &vector["name"];
        let decoded = decode(&bytes_of(&vector["bytes"]));
        assert_eq!(decoded, Err(error_of(&vector["error"])), "{name}");
    }
}
