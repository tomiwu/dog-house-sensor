//! The example Home Assistant files in `docs/home-assistant` (PRD 5.3, H2 and H3).
//! The tests read the files and check the syntax and the key settings.

use serde::de::DeserializeOwned;
use yaml_serde::Value;

fn load<T: DeserializeOwned>(file: &str) -> T {
    let path = format!(
        "{}/../docs/home-assistant/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    yaml_serde::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn text(value: &Value) -> String {
    yaml_serde::to_string(value).unwrap()
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn automations() -> Vec<Value> {
    load("automations.yaml")
}

fn automation(id: &str) -> Value {
    automations()
        .into_iter()
        .find(|a| a["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("automation {id} is missing"))
}

#[test]
fn helpers_allow_the_default_limits_and_keep_ui_changes() {
    let helpers: Value = load("helpers.yaml");
    for (key, default, unit) in [
        ("dog_house_temperature_low", 0.0, "°C"),
        ("dog_house_temperature_high", 30.0, "°C"),
        ("dog_house_battery_low", 3.5, "V"),
    ] {
        let helper = &helpers["input_number"][key];
        assert!(
            helper["initial"].is_null(),
            "{key}: initial resets the value at restart"
        );
        assert_eq!(helper["unit_of_measurement"].as_str(), Some(unit), "{key}");
        assert_eq!(helper["mode"].as_str(), Some("box"), "{key}");
        assert!(number(&helper["min"]) <= default, "{key}");
        assert!(number(&helper["max"]) >= default, "{key}");
        let steps = default / number(&helper["step"]);
        assert_eq!(
            steps,
            steps.round(),
            "{key}: the default is a multiple of step"
        );
    }
}

#[test]
fn there_is_one_automation_for_each_alarm() {
    let ids: Vec<Value> = automations().iter().map(|a| a["id"].clone()).collect();
    assert_eq!(
        ids,
        [
            "dog_house_temperature_low",
            "dog_house_temperature_high",
            "dog_house_battery_low",
            "dog_house_no_data",
        ]
    );
}

#[test]
fn every_alarm_waits_before_it_fires_and_notifies() {
    for alarm in automations() {
        let id = alarm["id"].as_str().expect("id");
        let triggers = alarm["triggers"].as_sequence().expect(id);
        assert!(!triggers.is_empty(), "{id}");
        for trigger in triggers {
            assert!(
                !trigger["for"].is_null(),
                "{id}: the trigger needs a for: time"
            );
        }
        let actions = text(&alarm["actions"]);
        assert!(actions.contains("notify.notify"), "{id}: {actions}");
    }
}

#[test]
fn limit_alarms_read_the_limit_from_a_helper() {
    for id in [
        "dog_house_temperature_low",
        "dog_house_temperature_high",
        "dog_house_battery_low",
    ] {
        let triggers = text(&automation(id)["triggers"]);
        assert!(
            triggers.contains(&format!("input_number.{id}")),
            "{triggers}"
        );
    }
}

#[test]
fn the_no_data_alarm_uses_unavailable_and_last_seen_for_45_minutes() {
    let triggers = text(&automation("dog_house_no_data")["triggers"]);
    for part in ["unavailable", "sensor.dog_house_last_seen", "45"] {
        assert!(triggers.contains(part), "{part}: {triggers}");
    }
}

#[test]
fn the_no_data_alarm_sends_one_message_when_both_triggers_fire() {
    let conditions = text(&automation("dog_house_no_data")["conditions"]);
    assert!(
        conditions.contains("this.attributes.last_triggered"),
        "{conditions}"
    );
}

#[test]
fn the_dashboard_shows_four_values_and_a_history_graph() {
    let card = text(&load("dashboard.yaml"));
    for part in [
        "sensor.dog_house_temperature",
        "sensor.dog_house_humidity",
        "sensor.dog_house_battery_voltage",
        "sensor.dog_house_last_seen",
        "history-graph",
    ] {
        assert!(card.contains(part), "{part}: {card}");
    }
}
