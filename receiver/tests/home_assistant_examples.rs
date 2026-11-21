//! The example Home Assistant files in `docs/home-assistant` (PRD 5.3, H2 and H3).
//! The tests read the files and check the syntax and the key settings.

use yaml_serde::Value;

fn load(file: &str) -> Value {
    let path = format!(
        "{}/../docs/home-assistant/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    yaml_serde::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn automations() -> Vec<Value> {
    load("automations.yaml")
        .as_sequence()
        .expect("a list of automations")
        .clone()
}

fn automation(id: &str) -> Value {
    automations()
        .into_iter()
        .find(|a| a["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("automation {id} is missing"))
}

#[test]
fn helpers_allow_the_default_limits_and_keep_ui_changes() {
    let helpers = load("helpers.yaml");
    let numbers = &helpers["input_number"];
    for (key, default, unit) in [
        ("dog_house_temperature_low", 0.0, "°C"),
        ("dog_house_temperature_high", 30.0, "°C"),
        ("dog_house_battery_low", 3.5, "V"),
    ] {
        let helper = &numbers[key];
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
    let ids: Vec<String> = automations()
        .iter()
        .map(|a| a["id"].as_str().expect("id").to_string())
        .collect();
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
        let id = alarm["id"].as_str().unwrap();
        let triggers = alarm["triggers"].as_sequence().expect(id);
        assert!(!triggers.is_empty(), "{id}");
        for trigger in triggers {
            assert!(
                !trigger["for"].is_null(),
                "{id}: the trigger needs a for: time"
            );
        }
        let text = yaml_serde::to_string(&alarm["actions"]).unwrap();
        assert!(text.contains("notify.notify"), "{id}: {text}");
    }
}

#[test]
fn limit_alarms_read_the_limit_from_a_helper() {
    for (id, helper) in [
        (
            "dog_house_temperature_low",
            "input_number.dog_house_temperature_low",
        ),
        (
            "dog_house_temperature_high",
            "input_number.dog_house_temperature_high",
        ),
        (
            "dog_house_battery_low",
            "input_number.dog_house_battery_low",
        ),
    ] {
        let text = yaml_serde::to_string(&automation(id)["triggers"]).unwrap();
        assert!(text.contains(helper), "{id}: {text}");
    }
}

#[test]
fn the_no_data_alarm_uses_unavailable_and_last_seen_for_45_minutes() {
    let alarm = automation("dog_house_no_data");
    let text = yaml_serde::to_string(&alarm["triggers"]).unwrap();
    assert!(text.contains("unavailable"), "{text}");
    assert!(text.contains("sensor.dog_house_last_seen"), "{text}");
    assert!(text.contains("45"), "{text}");
}

#[test]
fn the_dashboard_shows_four_values_and_a_history_graph() {
    let card = load("dashboard.yaml");
    let text = yaml_serde::to_string(&card).unwrap();
    for entity in [
        "sensor.dog_house_temperature",
        "sensor.dog_house_humidity",
        "sensor.dog_house_battery_voltage",
        "sensor.dog_house_last_seen",
    ] {
        assert!(text.contains(entity), "{entity}");
    }
    assert!(text.contains("history-graph"), "{text}");
}
