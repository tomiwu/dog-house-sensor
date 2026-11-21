# Home Assistant examples

Example files for the alarms and the dashboard of the dog house sensor.
HA keeps the history and runs the alarms (PRD D1).
The sensors come from MQTT discovery. You do not write YAML for them (PRD H1).

| File | Content |
|------|---------|
| `helpers.yaml` | `input_number` helpers for the three alarm limits |
| `automations.yaml` | Four alarms: low temperature, high temperature, low battery, no data |
| `dashboard.yaml` | One card: temperature, humidity, battery, last seen, history graph |

## Entity names

The files use the entity names that the bridge makes for the node with the
name `Dog house`:

- `sensor.dog_house_temperature`
- `sensor.dog_house_humidity`
- `sensor.dog_house_battery_voltage`
- `sensor.dog_house_last_seen`

If you use another node name, replace `dog_house` in the files.

The automations use the `triggers:` and `actions:` keys. They need
Home Assistant 2024.10 or later.

## How to add the files

1. Start the bridge. Check that the device `Dog house` is in HA.
2. Helpers. Copy the text of `helpers.yaml` into `configuration.yaml`, then
   restart HA. If you already have `input_number:` there, merge the three
   helpers into it. You can also make the three helpers in the UI
   (Settings > Devices & services > Helpers). Then set the entity ID of each
   helper to the ID in `helpers.yaml`, for example
   `input_number.dog_house_temperature_low`. The automations use these IDs.
3. Set the defaults once. Open Settings > Devices & services > Helpers and set:
   low temperature `0` °C, high temperature `30` °C, low battery `3.5` V.
   HA keeps these values after a restart. The files have no `initial` on
   purpose: it would reset your values at each restart.
4. Automations. Copy the text of `automations.yaml` into `automations.yaml` in
   the HA configuration folder (or use `automation: !include automations.yaml`),
   then reload the automations.
5. Change `notify.notify` in the automations to your own notification service,
   for example `notify.mobile_app_<your_phone>`.
6. Dashboard. Open the dashboard, select Edit, Add card, Manual. Paste the text
   of `dashboard.yaml`.

## Change a limit

Open Settings > Devices & services > Helpers (or add the helpers to a
dashboard), select the helper and set the new value. No YAML edit is needed.

## Alarm timing

Each alarm has a `for:` time, so a short spike does not send a message.

| Alarm | Condition must stay true for |
|-------|------------------------------|
| Low or high temperature | 30 minutes |
| Low battery | 1 hour |
| No data | 5 minutes, after 45 minutes without a packet |

The no data alarm has two triggers: the sensor is `unavailable` (the bridge
sets `expire_after` to 45 minutes), or the `last_seen` time is older than
45 minutes. When the bridge is offline, the sensor is `unavailable` at once,
so the alarm comes after 5 minutes. When both triggers fire, a condition
stops the second message: the alarm sends one message in 45 minutes.

## Check the files

```sh
yamllint docs/home-assistant
cargo test -p receiver --test home_assistant_examples
```

The test reads the files and checks the syntax, the default limits and the
alarms. It does not start HA.
