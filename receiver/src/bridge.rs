//! The MQTT side of the bridge: availability, discovery and state (PRD sections 5.2 and 7.7).

use std::fmt;

use protocol::Packet;
use serde_json::Value;
use time::OffsetDateTime;

use crate::config::Config;
use crate::payload;

/// Failure of an MQTT operation.
#[derive(Debug, PartialEq)]
pub enum MqttError {
    /// The client refused the request.
    Client(String),
    /// The reading is from a node that is not in the configuration.
    UnknownNode(u8),
}

impl fmt::Display for MqttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(text) => write!(f, "MQTT client error: {text}"),
            Self::UnknownNode(id) => write!(f, "node {id} is not in the configuration"),
        }
    }
}

impl std::error::Error for MqttError {}

/// The MQTT operations that the bridge needs. Tests use a fake.
pub trait Mqtt {
    /// Publish with QoS 1.
    fn publish(&mut self, topic: &str, payload: &str, retain: bool) -> Result<(), MqttError>;
    /// Subscribe with QoS 1.
    fn subscribe(&mut self, topic: &str) -> Result<(), MqttError>;
}

fn publish_json(
    mqtt: &mut dyn Mqtt,
    topic: &str,
    payload: &Value,
    retain: bool,
) -> Result<(), MqttError> {
    mqtt.publish(topic, &payload.to_string(), retain)
}

/// Publish the retained discovery message of each configured node.
fn publish_discovery(mqtt: &mut dyn Mqtt, config: &Config) -> Result<(), MqttError> {
    config.nodes.iter().try_for_each(|(id, node)| {
        let topic = payload::discovery_topic(&config.mqtt, *id);
        let body = payload::discovery_payload(&config.mqtt, *id, node);
        publish_json(mqtt, &topic, &body, true)
    })
}

/// Call this after each connect (a new session has no subscriptions).
pub fn on_connected(mqtt: &mut dyn Mqtt, config: &Config) -> Result<(), MqttError> {
    mqtt.subscribe(&payload::status_topic(&config.mqtt))?;
    mqtt.publish(
        &payload::availability_topic(&config.mqtt),
        payload::ONLINE,
        true,
    )?;
    publish_discovery(mqtt, config)
}

/// Call this for each message that arrives from the broker.
pub fn on_message(
    mqtt: &mut dyn Mqtt,
    config: &Config,
    topic: &str,
    body: &[u8],
) -> Result<(), MqttError> {
    let from_ha = topic == payload::status_topic(&config.mqtt);
    if from_ha && body == payload::ONLINE.as_bytes() {
        publish_discovery(mqtt, config)
    } else {
        Ok(())
    }
}

/// Publish the state of one reading.
pub fn publish_state(
    mqtt: &mut dyn Mqtt,
    config: &Config,
    packet: &Packet,
    last_seen: OffsetDateTime,
) -> Result<(), MqttError> {
    if !config.nodes.contains_key(&packet.node_id) {
        return Err(MqttError::UnknownNode(packet.node_id));
    }
    let topic = payload::state_topic(&config.mqtt, packet.node_id);
    publish_json(
        mqtt,
        &topic,
        &payload::state_payload(packet, last_seen),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::config::{DataRate, MqttConfig, NodeConfig, RadioConfig};
    use protocol::RadioSettings;
    use serde_json::Value;
    use time::macros::datetime;

    #[derive(Default)]
    struct Fake {
        published: Vec<(String, String, bool)>,
        subscribed: Vec<String>,
        fail: bool,
    }

    impl Mqtt for Fake {
        fn publish(&mut self, topic: &str, payload: &str, retain: bool) -> Result<(), MqttError> {
            if self.fail {
                return Err(MqttError::Client("queue full".into()));
            }
            self.published
                .push((topic.to_owned(), payload.to_owned(), retain));
            Ok(())
        }

        fn subscribe(&mut self, topic: &str) -> Result<(), MqttError> {
            self.subscribed.push(topic.to_owned());
            Ok(())
        }
    }

    fn config() -> Config {
        let node = |name: &str| NodeConfig {
            name: name.into(),
            reading_interval_s: 900,
            expire_after_s: 2700,
        };
        Config {
            radio: RadioConfig {
                spi_device: "/dev/spidev0.0".into(),
                gpio_chip: "/dev/gpiochip0".into(),
                ce_gpio: 25,
                settings: RadioSettings {
                    channel: 76,
                    pa_level_dbm: 0.0,
                    pa_gain_db: 0.0,
                    antenna_gain_dbi: 2.0,
                },
                allow_out_of_band: false,
                data_rate: DataRate::Kbps250,
                address: b"DHSN1".to_vec(),
            },
            mqtt: MqttConfig {
                host: "broker.invalid".into(),
                port: 1883,
                username: None,
                password: None,
                discovery_prefix: "homeassistant".into(),
                base_topic: "dog-house-sensor".into(),
            },
            nodes: BTreeMap::from([(1, node("Dog house")), (2, node("Kennel"))]),
        }
    }

    fn topics(fake: &Fake) -> Vec<&str> {
        fake.published.iter().map(|(t, _, _)| t.as_str()).collect()
    }

    #[test]
    fn connect_subscribes_and_publishes_online_then_discovery() {
        let mut fake = Fake::default();
        on_connected(&mut fake, &config()).unwrap();
        assert_eq!(fake.subscribed, ["homeassistant/status"]);
        assert_eq!(
            topics(&fake),
            [
                "dog-house-sensor/bridge/availability",
                "homeassistant/device/dog_house_1/config",
                "homeassistant/device/dog_house_2/config",
            ]
        );
        assert_eq!(fake.published[0].1, "online");
    }

    #[test]
    fn availability_and_discovery_are_retained() {
        let mut fake = Fake::default();
        on_connected(&mut fake, &config()).unwrap();
        assert!(fake.published.iter().all(|(_, _, retain)| *retain));
    }

    #[test]
    fn discovery_payload_is_valid_json_for_the_node() {
        let mut fake = Fake::default();
        on_connected(&mut fake, &config()).unwrap();
        let value: Value = serde_json::from_str(&fake.published[2].1).unwrap();
        assert_eq!(value["dev"]["name"], "Kennel");
    }

    #[test]
    fn ha_online_publishes_the_discovery_again_without_availability() {
        let mut fake = Fake::default();
        on_message(&mut fake, &config(), "homeassistant/status", b"online").unwrap();
        assert_eq!(
            topics(&fake),
            [
                "homeassistant/device/dog_house_1/config",
                "homeassistant/device/dog_house_2/config",
            ]
        );
    }

    #[test]
    fn ha_offline_and_other_messages_do_nothing() {
        let mut fake = Fake::default();
        let config = config();
        on_message(&mut fake, &config, "homeassistant/status", b"offline").unwrap();
        on_message(&mut fake, &config, "homeassistant/status", b"").unwrap();
        on_message(&mut fake, &config, "other/topic", b"online").unwrap();
        assert!(fake.published.is_empty());
    }

    #[test]
    fn state_goes_to_the_node_topic_without_retain() {
        let mut fake = Fake::default();
        let packet = Packet {
            node_id: 2,
            seq: 9,
            temp_dc: 215,
            hum_dpct: 634,
            vbat_mv: 3912,
            status: 0,
            prev_retries: 1,
        };
        let seen = datetime!(2026-11-21 17:22:35 UTC);
        publish_state(&mut fake, &config(), &packet, seen).unwrap();
        let (topic, payload, retain) = &fake.published[0];
        assert_eq!(topic, "dog-house-sensor/2/state");
        assert!(!retain);
        let value: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(value["temperature"], 21.5);
        assert_eq!(value["seq"], 9);
    }

    #[test]
    fn state_of_an_unknown_node_is_refused() {
        let mut fake = Fake::default();
        let packet = Packet {
            node_id: 9,
            seq: 0,
            temp_dc: 0,
            hum_dpct: 0,
            vbat_mv: 3000,
            status: 0,
            prev_retries: 0,
        };
        let seen = datetime!(2026-11-21 17:22:35 UTC);
        let result = publish_state(&mut fake, &config(), &packet, seen);
        assert_eq!(result, Err(MqttError::UnknownNode(9)));
        assert!(fake.published.is_empty());
    }

    #[test]
    fn a_client_error_is_returned() {
        let mut fake = Fake {
            fail: true,
            ..Fake::default()
        };
        assert!(on_connected(&mut fake, &config()).is_err());
    }
}
