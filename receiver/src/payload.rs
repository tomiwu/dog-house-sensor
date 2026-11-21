//! MQTT topics and JSON payloads (PRD section 7.7).

use protocol::Packet;
use serde_json::{Value, json};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::config::{MqttConfig, NodeConfig};

/// Payload of the availability topic while the bridge runs.
pub const ONLINE: &str = "online";
/// Payload of the availability topic after the bridge stops. Also the Last Will.
pub const OFFLINE: &str = "offline";

/// Topic of the bridge availability.
pub fn availability_topic(mqtt: &MqttConfig) -> String {
    format!("{}/bridge/availability", mqtt.base_topic)
}

/// Topic of the state of one node.
pub fn state_topic(mqtt: &MqttConfig, node_id: u8) -> String {
    format!("{}/{node_id}/state", mqtt.base_topic)
}

/// Topic where Home Assistant announces its start.
pub fn status_topic(mqtt: &MqttConfig) -> String {
    format!("{}/status", mqtt.discovery_prefix)
}

/// Topic of the device discovery message of one node.
pub fn discovery_topic(mqtt: &MqttConfig, node_id: u8) -> String {
    format!(
        "{}/device/{}/config",
        mqtt.discovery_prefix,
        object_id(node_id)
    )
}

fn object_id(node_id: u8) -> String {
    format!("dog_house_{node_id}")
}

/// The time as ISO 8601 text in UTC, without a fraction of a second.
fn iso_8601(time: OffsetDateTime) -> String {
    time.to_offset(UtcOffset::UTC)
        .replace_nanosecond(0)
        .ok()
        .and_then(|utc| utc.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// The state JSON of one reading.
pub fn state_payload(packet: &Packet, last_seen: OffsetDateTime) -> Value {
    json!({
        "temperature": f64::from(packet.temp_dc) / 10.0,
        "humidity": f64::from(packet.hum_dpct) / 10.0,
        "battery": f64::from(packet.vbat_mv) / 1000.0,
        "last_seen": iso_8601(last_seen),
        "seq": packet.seq,
        "retries": packet.prev_retries,
    })
}

/// One sensor component. `extra` holds the fields that differ between sensors.
fn component(
    mqtt: &MqttConfig,
    node_id: u8,
    node: &NodeConfig,
    key: &str,
    name: &str,
    extra: Value,
) -> Value {
    let mut value = json!({
        "p": "sensor",
        "unique_id": format!("{}_{key}", object_id(node_id)),
        "name": name,
        "state_topic": state_topic(mqtt, node_id),
        "value_template": format!("{{{{ value_json.{key} }}}}"),
        "expire_after": node.expire_after_s,
    });
    if let (Some(target), Value::Object(fields)) = (value.as_object_mut(), extra) {
        target.extend(fields);
    }
    value
}

/// The device discovery JSON of one node.
pub fn discovery_payload(mqtt: &MqttConfig, node_id: u8, node: &NodeConfig) -> Value {
    let part = |key: &str, name: &str, extra: Value| {
        (
            key.to_owned(),
            component(mqtt, node_id, node, key, name, extra),
        )
    };
    let components: serde_json::Map<String, Value> = [
        part(
            "temperature",
            "Temperature",
            json!({
                "device_class": "temperature",
                "state_class": "measurement",
                "unit_of_measurement": "°C",
            }),
        ),
        part(
            "humidity",
            "Humidity",
            json!({
                "device_class": "humidity",
                "state_class": "measurement",
                "unit_of_measurement": "%",
            }),
        ),
        part(
            "battery",
            "Battery voltage",
            json!({
                "device_class": "voltage",
                "state_class": "measurement",
                "unit_of_measurement": "V",
                "entity_category": "diagnostic",
            }),
        ),
        part(
            "last_seen",
            "Last seen",
            json!({
                "device_class": "timestamp",
                "entity_category": "diagnostic",
            }),
        ),
    ]
    .into_iter()
    .collect();
    json!({
        "dev": {
            "identifiers": [object_id(node_id)],
            "name": node.name,
            "manufacturer": "dog-house-sensor",
            "model": "nRF24L01+ sensor node",
        },
        "o": {
            "name": "dog-house-bridge",
            "sw_version": env!("CARGO_PKG_VERSION"),
        },
        "availability_topic": availability_topic(mqtt),
        "payload_available": ONLINE,
        "payload_not_available": OFFLINE,
        "cmps": components,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn mqtt() -> MqttConfig {
        MqttConfig {
            host: "broker.invalid".into(),
            port: 1883,
            username: None,
            password: None,
            discovery_prefix: "homeassistant".into(),
            base_topic: "dog-house-sensor".into(),
        }
    }

    fn node() -> NodeConfig {
        NodeConfig {
            name: "Dog house".into(),
            reading_interval_s: 900,
            expire_after_s: 2700,
        }
    }

    fn packet(temp_dc: i16, hum_dpct: u16, vbat_mv: u16, seq: u16, retries: u8) -> Packet {
        Packet {
            node_id: 1,
            seq,
            temp_dc,
            hum_dpct,
            vbat_mv,
            status: 0,
            prev_retries: retries,
        }
    }

    fn golden(text: &str) -> Value {
        serde_json::from_str(text).expect("golden file is valid JSON")
    }

    #[test]
    fn topics_follow_the_configuration() {
        let mqtt = mqtt();
        assert_eq!(
            availability_topic(&mqtt),
            "dog-house-sensor/bridge/availability"
        );
        assert_eq!(state_topic(&mqtt, 7), "dog-house-sensor/7/state");
        assert_eq!(status_topic(&mqtt), "homeassistant/status");
        assert_eq!(
            discovery_topic(&mqtt, 1),
            "homeassistant/device/dog_house_1/config"
        );
    }

    #[test]
    fn topics_use_a_custom_prefix_and_base() {
        let mqtt = MqttConfig {
            discovery_prefix: "ha".into(),
            base_topic: "lab".into(),
            ..mqtt()
        };
        assert_eq!(status_topic(&mqtt), "ha/status");
        assert_eq!(discovery_topic(&mqtt, 2), "ha/device/dog_house_2/config");
        assert_eq!(state_topic(&mqtt, 2), "lab/2/state");
    }

    #[test]
    fn state_matches_the_golden_file() {
        let seen = datetime!(2026-11-21 17:22:35 UTC);
        let state = state_payload(&packet(215, 634, 3912, 4242, 2), seen);
        assert_eq!(state, golden(include_str!("../fixtures/state_node1.json")));
    }

    #[test]
    fn state_with_negative_temperature_and_extreme_values_matches_the_golden_file() {
        let seen = datetime!(2026-01-02 03:04:05 UTC);
        let state = state_payload(&packet(-123, 1000, 3500, 0, 15), seen);
        assert_eq!(
            state,
            golden(include_str!("../fixtures/state_node1_frost.json"))
        );
    }

    #[test]
    fn last_seen_has_no_fraction_and_uses_utc() {
        let seen = datetime!(2026-11-21 18:22:35.987654 +1);
        let state = state_payload(&packet(0, 0, 3000, 1, 0), seen);
        assert_eq!(state["last_seen"], "2026-11-21T17:22:35Z");
    }

    #[test]
    fn discovery_matches_the_golden_file() {
        let payload = discovery_payload(&mqtt(), 1, &node());
        assert_eq!(
            payload,
            golden(include_str!("../fixtures/discovery_node1.json"))
        );
    }

    #[test]
    fn discovery_uses_expire_after_and_names_from_the_configuration() {
        let node = NodeConfig {
            name: "Kennel".into(),
            reading_interval_s: 60,
            expire_after_s: 300,
        };
        let payload = discovery_payload(&mqtt(), 3, &node);
        assert_eq!(payload["dev"]["name"], "Kennel");
        assert_eq!(payload["dev"]["identifiers"][0], "dog_house_3");
        for (key, component) in payload["cmps"].as_object().unwrap() {
            assert_eq!(component["expire_after"], 300, "{key}");
            assert_eq!(
                component["state_topic"], "dog-house-sensor/3/state",
                "{key}"
            );
            let id = format!("dog_house_3_{key}");
            assert_eq!(component["unique_id"], id.as_str());
        }
    }
}
