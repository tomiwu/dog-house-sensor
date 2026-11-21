//! The real MQTT connection, built on `rumqttc` (PRD section 7.6).

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use protocol::Packet;
use rumqttc::{
    Client, ClientError, Connection, Event, LastWill, MqttOptions, Packet as MqttPacket, QoS,
};
use time::OffsetDateTime;

use crate::bridge::{self, Mqtt, MqttError};
use crate::config::{Config, MqttConfig};
use crate::link::Links;
use crate::payload;

/// MQTT client identifier of the bridge.
const CLIENT_ID: &str = "dog-house-bridge";
const KEEP_ALIVE: Duration = Duration::from_secs(30);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// The queue holds the discovery messages of all 255 possible nodes.
const QUEUE_SIZE: usize = 512;

/// A decoded packet and the time of its arrival.
pub type Reading = (Packet, OffsetDateTime);

/// The connection options: credentials and the Last Will `offline`. A user
/// name without a password is valid in MQTT, so the password can be empty.
pub fn options(mqtt: &MqttConfig) -> MqttOptions {
    let mut options = MqttOptions::new(CLIENT_ID, &mqtt.host, mqtt.port);
    options.set_keep_alive(KEEP_ALIVE);
    options.set_last_will(LastWill::new(
        payload::availability_topic(mqtt),
        payload::OFFLINE,
        QoS::AtLeastOnce,
        true,
    ));
    if let Some(user) = &mqtt.username {
        let password = mqtt.password.as_ref().map_or("", AsRef::as_ref);
        options.set_credentials(user, password);
    }
    options
}

struct Link(Client);

fn client_error(error: ClientError) -> MqttError {
    MqttError::Client(error.to_string())
}

impl Mqtt for Link {
    fn publish(&mut self, topic: &str, body: &str, retain: bool) -> Result<(), MqttError> {
        self.0
            .try_publish(topic, QoS::AtLeastOnce, retain, body)
            .map_err(client_error)
    }

    fn subscribe(&mut self, topic: &str) -> Result<(), MqttError> {
        self.0
            .try_subscribe(topic, QoS::AtLeastOnce)
            .map_err(client_error)
    }
}

fn log_failure(result: Result<(), MqttError>) {
    if let Err(error) = result {
        eprintln!("error: {error}");
    }
}

fn publish_readings(link: &mut Link, config: &Config, readings: &Receiver<Reading>) {
    let mut links = Links::default();
    for (packet, seen) in readings {
        log_failure(bridge::publish_state(
            link, config, &mut links, &packet, seen,
        ));
    }
}

fn handle_event(link: &mut Link, config: &Config, event: Event) {
    match event {
        Event::Incoming(MqttPacket::ConnAck(_)) => {
            eprintln!("connected to the MQTT broker");
            log_failure(bridge::on_connected(link, config));
        }
        Event::Incoming(MqttPacket::Publish(message)) => {
            log_failure(bridge::on_message(
                link,
                config,
                &message.topic,
                &message.payload,
            ));
        }
        _ => {}
    }
}

fn run_events(link: &mut Link, config: &Config, mut connection: Connection) {
    for event in connection.iter() {
        match event {
            Ok(event) => handle_event(link, config, event),
            Err(error) => {
                eprintln!("MQTT connection error: {error}");
                thread::sleep(RECONNECT_DELAY);
            }
        }
    }
}

/// Connect and serve until the connection ends. The readings from the
/// channel go to the state topics. This function runs the event loop on the
/// calling thread and the readings on a second thread.
pub fn run(config: Config, readings: Receiver<Reading>) {
    let (client, connection) = Client::new(options(&config.mqtt), QUEUE_SIZE);
    let config = Arc::new(config);
    let sender_config = Arc::clone(&config);
    let mut sender_link = Link(client.clone());
    thread::spawn(move || publish_readings(&mut sender_link, &sender_config, &readings));
    run_events(&mut Link(client), &config, connection);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mqtt(username: Option<&str>, password: Option<&str>) -> MqttConfig {
        let text = format!(
            "[radio]\n[mqtt]\nhost = \"broker.invalid\"\nport = 1884\n{}\n\
             [nodes.1]\nname = \"A\"\nexpire_after_s = 2700\n",
            username.map_or(String::new(), |u| format!("username = \"{u}\""))
        );
        Config::from_toml(&text, password, &|_| unreachable!())
            .unwrap()
            .mqtt
    }

    #[test]
    fn options_have_the_broker_and_the_last_will() {
        let options = options(&mqtt(None, None));
        assert_eq!(options.broker_address(), ("broker.invalid".into(), 1884));
        let will = options.last_will().expect("a last will");
        assert_eq!(will.topic, "dog-house-sensor/bridge/availability");
        assert_eq!(&will.message[..], b"offline");
        assert!(will.retain);
    }

    #[test]
    fn options_have_the_credentials_from_the_configuration() {
        let options = options(&mqtt(Some("bridge-user"), Some("pw-for-test")));
        let login = options.credentials().expect("credentials");
        assert_eq!(login.username, "bridge-user");
        assert_eq!(login.password, "pw-for-test");
    }

    #[test]
    fn options_have_no_credentials_without_a_user_name() {
        assert!(
            options(&mqtt(None, Some("pw-for-test")))
                .credentials()
                .is_none()
        );
    }

    #[test]
    fn options_have_the_user_name_without_a_password() {
        let options = options(&mqtt(Some("bridge-user"), None));
        let login = options.credentials().expect("credentials");
        assert_eq!(login.username, "bridge-user");
        assert_eq!(login.password, "");
    }
}
