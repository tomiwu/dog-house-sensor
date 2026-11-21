//! Bridge configuration: TOML file, validation and secrets (PRD sections 7.1a and 8).

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;

use protocol::{RadioCheck, RadioSettings, Violation, check_radio};
use serde::Deserialize;

use crate::research::out_of_band_warning;

/// Name of the environment variable with the MQTT password.
pub const PASSWORD_ENV: &str = "DHS_MQTT_PASSWORD";

const MAX_CHANNEL: i64 = 125;
const PA_LEVELS_DBM: [i64; 4] = [-18, -12, -6, 0];

/// A configuration error: one text for each invalid value.
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// The file cannot be read.
    Read(String),
    /// The file is not valid TOML, or a field has the wrong type.
    Parse(String),
    /// One or more values are invalid.
    Invalid(Vec<String>),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(text) => write!(f, "cannot read the configuration: {text}"),
            Self::Parse(text) => write!(f, "cannot parse the configuration: {text}"),
            Self::Invalid(list) => {
                write!(f, "invalid configuration:")?;
                list.iter().try_for_each(|item| write!(f, "\n  - {item}"))
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// A secret text. The `Debug` output hides the value.
#[derive(Clone, PartialEq)]
pub struct Secret(String);

impl AsRef<str> for Secret {
    /// The secret value. Do not log it.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<hidden>)")
    }
}

/// Radio data rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataRate {
    Kbps250,
    Mbps1,
    Mbps2,
}

/// Valid radio settings.
#[derive(Clone, Debug, PartialEq)]
pub struct RadioConfig {
    pub spi_device: String,
    pub gpio_chip: String,
    pub ce_gpio: u8,
    pub settings: RadioSettings,
    pub allow_out_of_band: bool,
    pub data_rate: DataRate,
    pub address: Vec<u8>,
}

impl RadioConfig {
    /// The warning for accepted research settings, if one is needed.
    pub fn warning(&self) -> Option<&'static str> {
        out_of_band_warning(&self.settings, self.allow_out_of_band)
    }
}

/// Valid MQTT settings.
#[derive(Clone, Debug, PartialEq)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<Secret>,
    pub discovery_prefix: String,
    pub base_topic: String,
}

/// Valid settings of one node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeConfig {
    pub name: String,
    pub reading_interval_s: u32,
    pub expire_after_s: u32,
}

/// The valid configuration of the bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub radio: RadioConfig,
    pub mqtt: MqttConfig,
    pub nodes: BTreeMap<u8, NodeConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    radio: RawRadio,
    mqtt: RawMqtt,
    #[serde(default)]
    nodes: BTreeMap<String, RawNode>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawRadio {
    spi_device: String,
    gpio_chip: String,
    ce_gpio: i64,
    channel: i64,
    pa_level_dbm: i64,
    pa_gain_db: f64,
    antenna_gain_dbi: f64,
    allow_out_of_band: bool,
    data_rate: String,
    address: String,
}

impl Default for RawRadio {
    fn default() -> Self {
        Self {
            spi_device: "/dev/spidev0.0".into(),
            gpio_chip: "/dev/gpiochip0".into(),
            ce_gpio: 25,
            channel: 76,
            pa_level_dbm: 0,
            pa_gain_db: 0.0,
            antenna_gain_dbi: 2.0,
            allow_out_of_band: false,
            data_rate: "250kbps".into(),
            address: "DHSN1".into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMqtt {
    host: String,
    #[serde(default = "default_port")]
    port: i64,
    username: Option<String>,
    password_file: Option<String>,
    #[serde(default = "default_discovery_prefix")]
    discovery_prefix: String,
    #[serde(default = "default_base_topic")]
    base_topic: String,
}

fn default_port() -> i64 {
    1883
}

fn default_discovery_prefix() -> String {
    "homeassistant".into()
}

fn default_base_topic() -> String {
    "dog-house-sensor".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    name: String,
    #[serde(default = "default_reading_interval")]
    reading_interval_s: i64,
    expire_after_s: i64,
}

fn default_reading_interval() -> i64 {
    900
}

/// Keep the value of a field, or store its error.
fn field<T>(errors: &mut Vec<String>, result: Result<T, String>) -> Option<T> {
    result.map_err(|text| errors.push(text)).ok()
}

fn in_range(name: &str, value: i64, min: i64, max: i64) -> Result<i64, String> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!("{name} is {value}, it must be {min} to {max}"))
    }
}

fn finite(name: &str, value: f64) -> Result<f32, String> {
    let value = value as f32;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{name} must be a finite number"))
    }
}

fn parse_pa_level(value: i64) -> Result<f32, String> {
    if PA_LEVELS_DBM.contains(&value) {
        Ok(value as f32)
    } else {
        Err(format!(
            "pa_level_dbm is {value}, it must be one of -18, -12, -6, 0"
        ))
    }
}

fn parse_data_rate(value: &str) -> Result<DataRate, String> {
    match value {
        "250kbps" => Ok(DataRate::Kbps250),
        "1Mbps" => Ok(DataRate::Mbps1),
        "2Mbps" => Ok(DataRate::Mbps2),
        other => Err(format!(
            "data_rate is \"{other}\", it must be \"250kbps\", \"1Mbps\" or \"2Mbps\""
        )),
    }
}

fn parse_address(value: &str) -> Result<Vec<u8>, String> {
    let bytes = value.as_bytes();
    if (3..=5).contains(&bytes.len()) {
        Ok(bytes.to_vec())
    } else {
        Err(format!(
            "address has {} bytes, it must have 3 to 5 bytes",
            bytes.len()
        ))
    }
}

fn legal_error(settings: &RadioSettings) -> Option<String> {
    let RadioCheck::Violations(list) = check_radio(settings) else {
        return None;
    };
    let reasons: Vec<String> = list.iter().map(describe_violation).collect();
    Some(format!(
        "radio settings are not legal in Poland and the EU ({}). \
         Use legal values, or set allow_out_of_band = true for research use only",
        reasons.join("; ")
    ))
}

fn describe_violation(violation: Violation) -> String {
    match violation {
        Violation::ChannelAboveLimit { channel } => format!(
            "channel {channel} is above {} (2483.5 MHz)",
            protocol::MAX_LEGAL_CHANNEL
        ),
        Violation::EirpAboveLimit { eirp_dbm } => format!(
            "e.i.r.p. {eirp_dbm} dBm is above {} dBm (10 mW)",
            protocol::MAX_EIRP_DBM
        ),
    }
}

fn validate_radio(raw: RawRadio) -> Result<RadioConfig, Vec<String>> {
    let mut errors = Vec::new();
    let ce_gpio = field(&mut errors, in_range("ce_gpio", raw.ce_gpio, 0, 255));
    let channel = field(
        &mut errors,
        in_range("channel", raw.channel, 0, MAX_CHANNEL),
    );
    let pa_level = field(&mut errors, parse_pa_level(raw.pa_level_dbm));
    let pa_gain = field(&mut errors, finite("pa_gain_db", raw.pa_gain_db));
    let antenna = field(
        &mut errors,
        finite("antenna_gain_dbi", raw.antenna_gain_dbi),
    );
    let data_rate = field(&mut errors, parse_data_rate(&raw.data_rate));
    let address = field(&mut errors, parse_address(&raw.address));
    let (
        Some(ce_gpio),
        Some(channel),
        Some(pa_level_dbm),
        Some(pa_gain_db),
        Some(antenna_gain_dbi),
    ) = (ce_gpio, channel, pa_level, pa_gain, antenna)
    else {
        return Err(errors);
    };
    let settings = RadioSettings {
        channel: channel as u8,
        pa_level_dbm,
        pa_gain_db,
        antenna_gain_dbi,
    };
    if !raw.allow_out_of_band {
        errors.extend(legal_error(&settings));
    }
    match (data_rate, address, errors.is_empty()) {
        (Some(data_rate), Some(address), true) => Ok(RadioConfig {
            spi_device: raw.spi_device,
            gpio_chip: raw.gpio_chip,
            ce_gpio: ce_gpio as u8,
            settings,
            allow_out_of_band: raw.allow_out_of_band,
            data_rate,
            address,
        }),
        _ => Err(errors),
    }
}

fn validate_mqtt(raw: RawMqtt, password: Option<Secret>) -> Result<MqttConfig, String> {
    let port = in_range("mqtt port", raw.port, 1, 65535)?;
    Ok(MqttConfig {
        host: raw.host,
        port: port as u16,
        username: raw.username,
        password,
        discovery_prefix: raw.discovery_prefix,
        base_topic: raw.base_topic,
    })
}

fn validate_node(key: &str, raw: RawNode) -> Result<(u8, NodeConfig), String> {
    let id = key
        .parse::<u8>()
        .ok()
        .filter(|id| *id >= 1)
        .ok_or_else(|| format!("node key \"{key}\" must be a number from 1 to 255"))?;
    let interval = in_range(
        &format!("nodes.{key}.reading_interval_s"),
        raw.reading_interval_s,
        1,
        i64::from(u32::MAX),
    )?;
    let expire = in_range(
        &format!("nodes.{key}.expire_after_s"),
        raw.expire_after_s,
        1,
        i64::from(u32::MAX),
    )?;
    if expire <= interval {
        return Err(format!(
            "nodes.{key}.expire_after_s is {expire}, it must be above reading_interval_s ({interval})"
        ));
    }
    Ok((
        id,
        NodeConfig {
            name: raw.name,
            reading_interval_s: interval as u32,
            expire_after_s: expire as u32,
        },
    ))
}

fn validate_nodes(raw: BTreeMap<String, RawNode>) -> Result<BTreeMap<u8, NodeConfig>, Vec<String>> {
    let mut errors = Vec::new();
    let mut nodes = BTreeMap::new();
    for (key, node) in raw {
        if let Some((id, node)) = field(&mut errors, validate_node(&key, node)) {
            nodes.insert(id, node);
        }
    }
    if errors.is_empty() && nodes.is_empty() {
        errors.push("at least one [nodes.N] section is needed".into());
    }
    if errors.is_empty() {
        Ok(nodes)
    } else {
        Err(errors)
    }
}

/// Find the MQTT password. The environment variable wins over the file.
fn resolve_password(
    env_value: Option<&str>,
    password_file: Option<&str>,
    read_file: &dyn Fn(&Path) -> io::Result<String>,
) -> Result<Option<Secret>, String> {
    if let Some(value) = env_value.filter(|value| !value.is_empty()) {
        return Ok(Some(Secret(value.to_owned())));
    }
    let Some(path) = password_file else {
        return Ok(None);
    };
    let text = read_file(Path::new(path))
        .map_err(|error| format!("cannot read password_file \"{path}\": {}", error.kind()))?;
    let value = text.trim_end_matches(['\r', '\n']);
    if value.is_empty() {
        return Err(format!("password_file \"{path}\" is empty"));
    }
    Ok(Some(Secret(value.to_owned())))
}

impl Config {
    /// Read and validate the configuration file. The password comes from the
    /// environment variable [`PASSWORD_ENV`] or from `password_file`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| ConfigError::Read(format!("{}: {error}", path.display())))?;
        let env_value = std::env::var(PASSWORD_ENV).ok();
        Self::from_toml(&text, env_value.as_deref(), &|file: &Path| {
            std::fs::read_to_string(file)
        })
    }

    /// Parse and validate a configuration text. `read_file` reads the password file.
    pub fn from_toml(
        text: &str,
        env_password: Option<&str>,
        read_file: &dyn Fn(&Path) -> io::Result<String>,
    ) -> Result<Self, ConfigError> {
        let raw: RawConfig =
            toml::from_str(text).map_err(|error| ConfigError::Parse(error.to_string()))?;
        let mut errors = Vec::new();
        let password = field(
            &mut errors,
            resolve_password(env_password, raw.mqtt.password_file.as_deref(), read_file),
        );
        let radio = validate_radio(raw.radio)
            .map_err(|list| errors.extend(list))
            .ok();
        let mqtt = field(&mut errors, validate_mqtt(raw.mqtt, password.flatten()));
        let nodes = validate_nodes(raw.nodes)
            .map_err(|list| errors.extend(list))
            .ok();
        match (radio, mqtt, nodes) {
            (Some(radio), Some(mqtt), Some(nodes)) if errors.is_empty() => {
                Ok(Self { radio, mqtt, nodes })
            }
            _ => Err(ConfigError::Invalid(errors)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::OUT_OF_BAND_WARNING;

    fn no_file(_: &Path) -> io::Result<String> {
        Err(io::Error::new(io::ErrorKind::NotFound, "missing"))
    }

    fn secret_text(secret: Option<Secret>) -> String {
        secret.expect("a password").as_ref().to_owned()
    }

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::from_toml(text, None, &no_file)
    }

    const EXPIRE: &str = "expire_after_s = 2700";

    fn config_text(radio: &str, mqtt: &str, node: &str) -> String {
        format!(
            "[radio]\n{radio}\n[mqtt]\nhost = \"broker.invalid\"\n{mqtt}\n\
             [nodes.1]\nname = \"Dog house\"\n{node}\n"
        )
    }

    fn base(radio: &str) -> String {
        config_text(radio, "", EXPIRE)
    }

    fn with_mqtt(mqtt: &str) -> String {
        config_text("", mqtt, EXPIRE)
    }

    fn with_node(node: &str) -> String {
        config_text("", "", node)
    }

    fn errors_of(text: &str) -> Vec<String> {
        match parse(text) {
            Err(ConfigError::Invalid(list)) => list,
            other => panic!("expected invalid configuration, got {other:?}"),
        }
    }

    fn assert_error_contains(text: &str, part: &str) {
        let errors = errors_of(text);
        assert!(
            errors.iter().any(|item| item.contains(part)),
            "{part:?} not in {errors:?}"
        );
    }

    #[test]
    fn example_file_is_valid() {
        let text = include_str!("../config.example.toml");
        let config = Config::from_toml(text, None, &|_| Ok("secret\n".into())).unwrap();
        assert_eq!(config.radio.settings.channel, 76);
        assert_eq!(config.radio.address, b"DHSN1");
        assert_eq!(config.radio.warning(), None);
        assert_eq!(config.mqtt.port, 1883);
        assert_eq!(config.nodes[&1].expire_after_s, 2700);
    }

    #[test]
    fn defaults_are_legal() {
        let config = parse(&base("")).unwrap();
        let radio = &config.radio;
        assert_eq!(radio.settings.channel, 76);
        assert_eq!(radio.settings.pa_level_dbm, 0.0);
        assert_eq!(radio.settings.pa_gain_db, 0.0);
        assert_eq!(radio.settings.antenna_gain_dbi, 2.0);
        assert!(!radio.allow_out_of_band);
        assert_eq!(radio.data_rate, DataRate::Kbps250);
        assert_eq!(config.nodes[&1].reading_interval_s, 900);
        assert_eq!(config.mqtt.password, None);
    }

    #[test]
    fn channels_84_and_108_are_rejected_without_the_flag() {
        for channel in [84, 108] {
            assert_error_contains(
                &base(&format!("channel = {channel}")),
                &format!("channel {channel} is above 83"),
            );
        }
    }

    #[test]
    fn channels_84_and_108_are_accepted_with_the_flag_and_warn() {
        for channel in [84, 108] {
            let config = parse(&base(&format!(
                "channel = {channel}\nallow_out_of_band = true"
            )))
            .unwrap();
            assert_eq!(config.radio.warning(), Some(OUT_OF_BAND_WARNING));
        }
    }

    #[test]
    fn channel_above_125_is_always_rejected() {
        assert_error_contains(
            &base("channel = 126\nallow_out_of_band = true"),
            "channel is 126, it must be 0 to 125",
        );
        assert_error_contains(&base("channel = -1"), "channel is -1");
    }

    #[test]
    fn zero_dbm_with_2_dbi_is_legal() {
        let config = parse(&base("pa_level_dbm = 0\nantenna_gain_dbi = 2")).unwrap();
        assert_eq!(config.radio.warning(), None);
    }

    #[test]
    fn pa_lna_gain_is_rejected_without_the_flag() {
        assert_error_contains(
            &base("pa_level_dbm = 0\npa_gain_db = 20"),
            "e.i.r.p. 22 dBm is above 10 dBm",
        );
    }

    #[test]
    fn pa_lna_gain_is_accepted_with_the_flag_and_warns() {
        let config = parse(&base("pa_gain_db = 20\nallow_out_of_band = true")).unwrap();
        assert_eq!(config.radio.warning(), Some(OUT_OF_BAND_WARNING));
    }

    #[test]
    fn gain_can_be_a_float() {
        assert!(parse(&base("pa_gain_db = 1.5\nantenna_gain_dbi = 0.5")).is_ok());
    }

    #[test]
    fn gain_that_is_not_finite_is_rejected_even_with_the_flag() {
        assert_error_contains(
            &base("pa_gain_db = inf\nallow_out_of_band = true"),
            "pa_gain_db must be a finite number",
        );
        assert_error_contains(
            &base("antenna_gain_dbi = nan\nallow_out_of_band = true"),
            "antenna_gain_dbi must be a finite number",
        );
    }

    #[test]
    fn gain_that_overflows_f32_is_rejected_even_with_the_flag() {
        assert_error_contains(
            &base("pa_gain_db = 1e300\nallow_out_of_band = true"),
            "pa_gain_db must be a finite number",
        );
    }

    #[test]
    fn pa_level_must_be_one_of_the_chip_levels() {
        for level in [-18, -12, -6, 0] {
            assert!(parse(&base(&format!("pa_level_dbm = {level}"))).is_ok());
        }
        assert_error_contains(&base("pa_level_dbm = -3"), "pa_level_dbm is -3");
        assert_error_contains(&base("pa_level_dbm = 4"), "pa_level_dbm is 4");
    }

    #[test]
    fn data_rate_is_checked() {
        for rate in ["250kbps", "1Mbps", "2Mbps"] {
            assert!(parse(&base(&format!("data_rate = \"{rate}\""))).is_ok());
        }
        assert_error_contains(&base("data_rate = \"500kbps\""), "data_rate is \"500kbps\"");
    }

    #[test]
    fn address_must_have_3_to_5_bytes() {
        for address in ["ABC", "ABCD", "ABCDE"] {
            assert!(parse(&base(&format!("address = \"{address}\""))).is_ok());
        }
        assert_error_contains(&base("address = \"AB\""), "address has 2 bytes");
        assert_error_contains(&base("address = \"ABCDEF\""), "address has 6 bytes");
    }

    #[test]
    fn mqtt_port_must_be_1_to_65535() {
        assert!(parse(&with_mqtt("port = 1")).is_ok());
        assert!(parse(&with_mqtt("port = 65535")).is_ok());
        assert_error_contains(&with_mqtt("port = 0"), "mqtt port is 0");
        assert_error_contains(&with_mqtt("port = 65536"), "mqtt port is 65536");
    }

    #[test]
    fn expire_after_must_be_above_the_reading_interval() {
        assert!(parse(&with_node("expire_after_s = 901")).is_ok());
        assert_error_contains(
            &with_node("expire_after_s = 900"),
            "nodes.1.expire_after_s is 900, it must be above reading_interval_s (900)",
        );
        assert_error_contains(
            &with_node("reading_interval_s = 60\nexpire_after_s = 30"),
            "must be above reading_interval_s (60)",
        );
    }

    #[test]
    fn node_key_must_be_a_node_number() {
        let text =
            "[radio]\n[mqtt]\nhost = \"h\"\n[nodes.dog]\nname = \"A\"\nexpire_after_s = 2700\n";
        assert_error_contains(text, "node key \"dog\"");
        let zero = text.replace("dog", "0");
        assert_error_contains(&zero, "node key \"0\"");
    }

    #[test]
    fn at_least_one_node_is_needed() {
        let text = "[radio]\n[mqtt]\nhost = \"h\"\n";
        assert_error_contains(text, "at least one [nodes.N] section");
    }

    #[test]
    fn all_errors_are_reported_together() {
        let errors = errors_of(&base("channel = 200\ndata_rate = \"x\"\naddress = \"A\""));
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn unknown_field_is_a_parse_error() {
        let text = base("chanel = 76");
        assert!(matches!(parse(&text), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn broken_toml_is_a_parse_error() {
        assert!(matches!(parse("[radio"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn password_comes_from_the_environment() {
        let config = Config::from_toml(&with_mqtt(""), Some("env-secret"), &no_file).unwrap();
        assert_eq!(secret_text(config.mqtt.password), "env-secret");
    }

    #[test]
    fn password_comes_from_the_file_without_the_line_end() {
        let text = with_mqtt("password_file = \"/run/pw\"");
        let read = |path: &Path| {
            assert_eq!(path, Path::new("/run/pw"));
            Ok("file-secret\n".to_owned())
        };
        let config = Config::from_toml(&text, None, &read).unwrap();
        assert_eq!(secret_text(config.mqtt.password), "file-secret");
    }

    #[test]
    fn environment_wins_over_the_file() {
        let text = with_mqtt("password_file = \"/run/pw\"");
        let config = Config::from_toml(&text, Some("env-secret"), &no_file).unwrap();
        assert_eq!(secret_text(config.mqtt.password), "env-secret");
    }

    #[test]
    fn missing_or_empty_password_file_is_an_error() {
        let text = with_mqtt("password_file = \"/run/pw\"");
        assert_error_contains(&text, "cannot read password_file \"/run/pw\"");
        let empty = Config::from_toml(&text, None, &|_| Ok("\n".into()));
        assert!(matches!(empty, Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn password_is_never_in_debug_or_error_text() {
        let text = with_mqtt("password_file = \"/run/pw\"");
        let config = Config::from_toml(&text, Some("topsecret"), &no_file).unwrap();
        assert!(!format!("{config:?}").contains("topsecret"));
        let bad = Config::from_toml(&base("channel = 200"), Some("topsecret"), &no_file);
        assert!(!bad.unwrap_err().to_string().contains("topsecret"));
    }

    #[test]
    fn load_reports_a_missing_file() {
        let error = Config::load(Path::new("/nonexistent/dhs/config.toml")).unwrap_err();
        assert!(matches!(error, ConfigError::Read(_)));
    }

    #[test]
    fn error_text_lists_each_problem_on_its_own_line() {
        let text = ConfigError::Invalid(vec!["a".into(), "b".into()]).to_string();
        assert_eq!(text, "invalid configuration:\n  - a\n  - b");
    }
}
