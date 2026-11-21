# Dog house sensor: Product Requirements Document (PRD)

Status: approved design, 2026-11-21. Owner: Tomasz Wu.
Language: ASD-STE100 Simplified Technical English.

## 1. Purpose

Measure the air temperature and the humidity at the dog house. Show the values in Home Assistant (HA). Send an alarm when the temperature is too low or too high, when the battery is low, or when the sensor stops sending data.

There is no Wi-Fi at the dog house. The sensor node uses a 2.4 GHz nRF24L01+ radio link to a receiver in the house.

## 2. Scope

### 2.1 In scope

- A battery node in the dog house: Wemos D1 mini, AM2302 (DHT22) sensor, nRF24L01+ radio, one 18650 Li-ion cell.
- A receiver in the house: Raspberry Pi 3 or 4 with a second nRF24L01+ module.
- A Rust bridge program on the Pi. It receives the radio packets and publishes them to an MQTT broker with HA MQTT discovery.
- A Mosquitto MQTT broker on the home NVR machine, next to the existing HA container (needs the approval of Tomasz before any change on the NVR, see section 9).
- Example HA automations for the alarms. The alarm limits are configurable.

### 2.2 Out of scope (for now)

- The enclosure.
- Photos and the camera.
- A Telegram bot in the bridge program. HA can send notifications (HA mobile app or the HA Telegram integration). This is an optional item (issue list, priority low).

## 3. Decisions from the grill (2026-11-21)

| # | Decision |
|---|----------|
| D1 | Data goes through Home Assistant. HA shows the dashboard, keeps the history, and runs the alarms. |
| D2 | Radio: nRF24L01+ on 2.4 GHz, Enhanced ShockBurst (auto-acknowledge and auto-retransmit), RF24-compatible on-air format. No Wi-Fi at the dog house. |
| D3 | Node board: Wemos D1 mini v3 (ESP8266EX, 4 MB flash). Deep sleep with D0 (GPIO16) connected to RST. One reading each 15 minutes. |
| D4 | Sensor: AM2302 (DHT22), bare 4-pin version, 10 kOhm pull-up resistor on the data pin. Pin 3 is not connected. |
| D5 | Power: one 18650 Li-ion cell with a low-loss regulator. |
| D6 | Receiver: Raspberry Pi 3 or 4, second nRF24L01+ on SPI0. |
| D7 | Language: Rust wherever possible. The Pi bridge is Rust. The node is Rust only if the spike (issue "Spike: Rust on the D1 mini") passes. If the spike fails, the node uses C++ (Arduino framework with PlatformIO). |
| D8 | Alarm limits are configurable. Defaults: temperature below 0 °C, temperature above 30 °C, low battery. |
| D9 | Repository: public, `tomiwu/dog-house-sensor`. Work is planned in GitHub issues with priority labels. |

Alternative, not approved: an ESP32-C3 board runs current Rust (`esp-hal`) with the standard tools. This needs a purchase. Tomasz must approve any purchase.

## 4. System overview

```
 Dog house (battery)                     House (USB power)              NVR (existing)
 +------------------------+   2.4 GHz    +------------------------+     +----------------------+
 | Wemos D1 mini          |  nRF24 ESB   | Raspberry Pi 3/4       | LAN | Mosquitto (new)      |
 | AM2302 (DHT22)         | -----------> | nRF24L01+ on SPI0      | --> | port 1883, user+pass |
 | nRF24L01+              |   auto-ack   | Rust bridge (systemd)  |MQTT | Home Assistant       |
 | 18650 + LDO            |              |                        |     | (container, existing)|
 +------------------------+              +------------------------+     +----------------------+
```

Data flow:

1. The node wakes up each 15 minutes.
2. The node reads the AM2302, reads the battery voltage, and sends one packet.
3. The nRF24L01+ sends the packet again automatically until it gets an acknowledgement or the retry limit occurs.
4. The node goes back to deep sleep.
5. The bridge on the Pi receives the packet, checks it, and publishes the values to MQTT.
6. HA finds the device through MQTT discovery and shows the sensors.
7. HA automations send the alarms.

## 5. Requirements

### 5.1 Node (dog house)

| ID | Requirement |
|----|-------------|
| N1 | The node must wake up each 15 minutes (configurable at build time). |
| N2 | The node must read temperature and humidity from the AM2302. It must obey the sensor timing (section 7.4). |
| N3 | The node must measure the battery voltage on A0 through a voltage divider (section 7.5). |
| N4 | The node must send one packet with: format version, node ID, sequence number, temperature, humidity, battery voltage, sensor status, and the retry count of the previous packet. |
| N5 | The node must use the nRF24L01+ at a power level of 0 dBm or lower, at 250 kbps, on a channel from 0 to 83 (2400 to 2483 MHz). |
| N6 | The node must put the nRF24L01+ in power-down mode and the sensor power off before deep sleep. |
| N7 | The node must not use Wi-Fi. The Wi-Fi radio stays off (for the ESP8266, wake with RF disabled). |
| N8 | If the sensor read fails, the node must still send a packet with the error status and the battery voltage. |

### 5.2 Bridge (Raspberry Pi, Rust)

| ID | Requirement |
|----|-------------|
| B1 | The bridge must receive packets from the nRF24L01+ on SPI0 with the same channel, data rate, address and CRC as the node. |
| B2 | The bridge must decode and validate each packet (length, version, value ranges). It must drop bad packets and log them. |
| B3 | The bridge must publish the values to MQTT as JSON on one state topic for each node. |
| B4 | The bridge must publish an HA MQTT device discovery message for each node: temperature, humidity, battery voltage, last seen, and link quality (lost packets, retries). |
| B5 | The bridge must publish its own availability with an MQTT Last Will message, and publish the discovery messages again when HA sends `online` on `homeassistant/status`. |
| B6 | The bridge must read all settings from a TOML configuration file. Secrets (the MQTT password) must come from an environment variable or a file outside the repository. |
| B7 | The bridge must reject a radio channel above 83 at start (regulatory limit, section 7.1). |
| B8 | The bridge must count lost packets from gaps in the sequence numbers. |
| B9 | The bridge must run as a systemd service and start again after a failure. |

### 5.3 Home Assistant

| ID | Requirement |
|----|-------------|
| H1 | The sensors must appear in HA automatically through MQTT discovery. No manual YAML for the entities. |
| H2 | The alarm limits must be configurable in HA (input_number helpers). Defaults: low temperature 0 °C, high temperature 30 °C, low battery 3.5 V (proposal, see 7.5). |
| H3 | Proposal: a "no data" alarm when no packet arrives for 45 minutes (3 missed readings). The sensors also use `expire_after` so that they show "unavailable" when data stops. |
| H4 | Notifications through the HA mobile app or the HA Telegram integration are optional (priority low). |

## 6. Packet format (proposal)

One Enhanced ShockBurst payload, 12 bytes, little-endian. The nRF24L01+ adds a 16-bit CRC.

| Offset | Size | Field | Unit / notes |
|--------|------|-------|--------------|
| 0 | 1 | `version` | Format version, starts at 1 |
| 1 | 1 | `node_id` | 1 for the dog house |
| 2 | 2 | `seq` | u16, increments each wake, kept in RTC memory |
| 4 | 2 | `temp_dc` | i16, 0.1 °C |
| 6 | 2 | `hum_dpct` | u16, 0.1 % RH |
| 8 | 2 | `vbat_mv` | u16, mV |
| 10 | 1 | `status` | bit 0: sensor read error, bit 1: checksum error, bit 2: first boot |
| 11 | 1 | `prev_retries` | auto-retransmit count of the previous packet (0 to 15) |

The `protocol` crate (`no_std`) owns this format. It has encode and decode functions and golden test vectors. If the node uses C++, the C++ code must pass the same golden test vectors.

## 7. Research and facts

### 7.1 Radio rules in Poland and the EU (2.4 GHz)

- ERC Recommendation 70-03, Annex 1, entry i: non-specific short range devices in 2400 to 2483.5 MHz, maximum 10 mW e.i.r.p., no duty cycle requirement. Harmonised standard: ETSI EN 300 440. Source: https://docdb.cept.org/download/4863 (edition of 17 October 2025, corrected 4 December 2025).
- Annex 3 of the same recommendation allows 100 mW e.i.r.p. for wideband data transmission systems (ETSI EN 300 328, for example Wi-Fi). The strict limit for this project is 10 mW e.i.r.p.
- Poland: the Regulation of the Minister of Digital Affairs of 9 February 2022 (Dz.U. 2022 poz. 567) lists the radio devices that you can use without a radio permit. Annex 1 covers general short range devices and Annex 2 covers wideband data transmission systems. These annexes follow ERC Recommendation 70-03. The annex tables are only in the original PDF; the 10 mW e.i.r.p. value for 2400 to 2483.5 MHz in Annex 1 is from ERC 70-03 and is not yet checked against the PDF table (to do in the issue "Bridge: configuration file and radio settings check"). Source: https://eli.gov.pl/api/acts/DU/2022/567/text.html and UKE: https://bip.uke.gov.pl/pozwolenia-radiowe/urzadzenia-niewymagajace-pozwolenia-/
- The nRF24L01+ maximum output power is 0 dBm (1 mW). A PCB antenna has a gain of approximately 0 to 2 dBi (estimate). Thus the e.i.r.p. is approximately 1 to 1.6 mW. This is less than 10 mW. Source: nRF24L01+ Product Specification v1.0, summary at https://pcbwiki.com/parts/nrf24l01-plus
- WARNING: the nRF24L01+ can tune from 2400 to 2525 MHz (channels 0 to 125). Many examples use channel 108 (2508 MHz). This is OUTSIDE the permitted band. Use only channels 0 to 83. The bridge rejects other values (B7).
- WARNING: do not use a PA+LNA nRF24 module at full power without a new check. Its output can be more than 10 mW e.i.r.p.
- The band is shared with Wi-Fi, Bluetooth and microwave ovens. Interference is possible. The channel is configurable.

### 7.2 nRF24L01+ facts and range

- GFSK modulation. Air data rates 250 kbps, 1 Mbps, 2 Mbps. Payload 1 to 32 bytes. Auto-acknowledge and auto-retransmit (Enhanced ShockBurst).
- Receiver sensitivity: -94 dBm at 250 kbps, -85 dBm at 1 Mbps, -82 dBm at 2 Mbps. Thus 250 kbps gives the longest range.
- Current: 11.3 mA TX at 0 dBm, 12.6 mA RX at 250 kbps, 26 µA standby-I, 900 nA power-down. Start-up from power-down: max 1.5 ms.
- Supply 1.9 to 3.6 V. Logic inputs are 5 V tolerant.
- Source: https://pcbwiki.com/parts/nrf24l01-plus and https://www.nicerf.com/news/nrf24l01.html
- Range with the PCB antenna: approximately 50 to 100 m in open space at 250 kbps. Walls and metal make the range much shorter. Source: https://lastminuteengineers.com/nrf24l01-arduino-wireless-communication/
- Practical tips: put a 10 µF to 100 µF capacitor (or minimum 1 µF ceramic) across VCC and GND at the module. Keep the antenna away from metal and from the board ground. Source: https://blog.adafruit.com/2021/01/28/nrf24l01-manually-calibrating-the-antenna-ncrmnt/
- The nRF24L01+ has no RSSI. It has only RPD (received power above -64 dBm). Thus link quality uses the retry count and the lost packet count.
- Risk: the distance and the walls between the dog house and the Pi are not known. Range test: issue "Link quality and range test".

### 7.3 Rust on the ESP8266 (D1 mini)

- `esp8266-hal`: last release 0.5.1 on 2022-07-19. The GitHub repository `esp-rs/esp8266-hal` is ARCHIVED (last push 2024-02-05). The README says: "this HAL is deprecated, no further development is planned". It uses `embedded-hal` 0.2 (not 1.0). Sources: https://crates.io/crates/esp8266-hal and https://github.com/esp-rs/esp8266-hal
- Toolchain: the ESP8266 needs the `xtensa-esp8266-none-elf` target from the esp-rs Rust fork, installed with `espup`. The target was removed in fork 1.81 and added again in 1.82. The maintainer said: "we likely won't be upstreaming the 8266 target, so once the Xtensa LLVM upstreaming is complete you'll be stuck on the latest version of this fork". Source: https://github.com/esp-rs/rust/issues/237
- Current fork release: v1.100.0.0 (2026-11-10). Current `espup`: v0.19.0 (2026-10-27). Source: https://github.com/esp-rs/rust-build/releases and https://github.com/esp-rs/espup/releases
- `espup --targets` lists only ESP32-family targets. Some users install an older fork (1.80.0.0) for the ESP8266. Source: https://andreas-mausch.de/blog/2024-10-20-esp01s/
- No Wi-Fi in Rust on the ESP8266. This is acceptable, because the node does not use Wi-Fi.
- Conclusion: Rust on the D1 mini is possible but has a high risk. The spike decides. The spike result goes in `docs/decisions/0001-node-language.md`.
- Alternative (needs a purchase): ESP32-C3 with `esp-hal` (official, maintained, standard RISC-V target).

### 7.4 AM2302 (DHT22) timing

- Start signal: the host pulls the data line low for minimum 0.8 ms (typical 1 ms, maximum 20 ms), then releases it.
- Sensor response: 80 µs low, then 80 µs high.
- 40 data bits, most significant bit first: humidity (16 bits), temperature (16 bits, bit 15 = negative), checksum (8 bits).
- Each bit: approximately 50 µs low, then high for 26 to 28 µs (bit 0) or 70 µs (bit 1). A sample point at approximately 40 µs after the rising edge separates the two.
- Minimum 2 s between two reads. After power-on, wait minimum 2 s.
- The value that you read is the PREVIOUS measurement. After a long sleep, read two times and use the second value.
- Thus each wake takes approximately 2.5 s. The bit timing needs microsecond accuracy. Disable interrupts during the 40-bit read.
- Source: https://www.mouser.com/datasheet/2/830/AM2302_manual-2487540.pdf

### 7.5 Battery life (ESTIMATE)

Facts:

- D1 mini deep sleep current, measured by others: approximately 150 to 230 µA on good v3 boards. Most of this is the CH340 USB chip and the onboard regulator (ME6211, approximately 60 µA, or RT9013, 25 to 50 µA). Sources: https://forum.arduino.cc/t/using-aa-batteries-to-power-d1-mini-wemos-board/1000129/12 and https://tech-ecke.de/arduino/arduino.htm
- Some clones with an unmarked USB chip use 6 to 13 mA in deep sleep. Source: https://kevinstadler.github.io/notes/wemos-d1-mini-clones-sleep-mode-current-power-consumption-esp8266/ and https://github.com/esp8266/Arduino/issues/6318
- A bare ESP-12 module uses approximately 16 to 20 µA in deep sleep (same sources).
- MCP1700 low-loss regulator: quiescent current 1.6 µA typical, 4 µA maximum, dropout 178 mV typical at 250 mA. Source: http://ww1.microchip.com/downloads/en/DeviceDoc/MCP1700-Low-Quiescent-Current-LDO-20001826E.pdf

Assumptions (all ESTIMATES):

- 18650 cell: 3000 mAh nominal, 2400 mAh usable (80 %). Cold weather makes this smaller.
- Self-discharge: approximately 2 % per month, 1.6 mAh/day.
- Each wake: 2.5 s at 20 mA average (RF off, sensor wait, one nRF24 packet). 96 wakes each day. Thus 2.5 x 20 / 3600 x 96 = 1.33 mAh/day.
- Battery divider on A0: approximately 9 µA (4.2 V across approximately 450 kOhm).

| Case | Sleep current | Sleep mAh/day | Total mAh/day | Life (estimate) |
|------|---------------|---------------|---------------|-----------------|
| A: good D1 mini v3, no change | 180 µA | 4.32 | 7.25 | approximately 330 days (11 months) |
| A-high: worse D1 mini v3 | 240 µA | 5.76 | 8.69 | approximately 275 days (9 months) |
| B: bad clone USB chip | 6 mA | 144 | 147 | approximately 16 days |
| C: CH340 power cut, external MCP1700 | 32 µA | 0.77 | 3.70 | approximately 650 days (21 months) |

Calculation: life = 2400 mAh / (total mAh/day). Total = sleep + 1.33 (wakes) + 1.6 (self-discharge).

Notes:

- Measure the real deep sleep current of the board of Tomasz first (issue "Battery measurement"). Case B is possible.
- Voltage: the AM2302 needs 3.3 to 5.5 V. When the cell is below approximately 3.5 V, the 3.3 V rail falls and the sensor can fail. Proposal: low battery alarm at 3.5 V cell voltage. Change this value after the measurement.
- The D1 mini A0 pin already has a divider (220 kOhm / 100 kOhm, 3.2 V full scale). To measure up to 4.2 V, add a resistor in series (for example 130 kOhm, full scale approximately 4.5 V). Calibrate in software.
- Do not charge a Li-ion cell below 0 °C. Charge the cell in the house.

### 7.6 Rust crates for the bridge (Raspberry Pi)

- `rf24-rs` 0.3.1 (2025-07-22): pure Rust nRF24L01 driver, `embedded-hal` 1.0, on-air compatible with the RF24 C++ library. Use with `linux-embedded-hal` 0.4 (features `spi`, `gpio-cdev`). Sources: https://crates.io/crates/rf24-rs and https://github.com/nRF24/rf24-rs
- `nrf24l01` 0.2.0: older Linux-only driver (spidev + sysfs GPIO, optional `rppal`). Not selected, because it is old and uses sysfs GPIO. Source: https://github.com/rtxm/rust-nrf24l01
- `rppal`: Raspberry Pi GPIO/SPI access. SPI0 pins: MOSI GPIO10 (pin 19), MISO GPIO9 (pin 21), SCLK GPIO11 (pin 23), CE0 GPIO8 (pin 24). Enable SPI with `dtparam=spi=on`. Source: https://docs.rs/rppal/latest/rppal/spi/index.html
- `rumqttc` 0.25.1 (2025-11-21): MQTT client. Source: https://crates.io/crates/rumqttc
- `teloxide` 0.17.0 (2025-07-11): Telegram bot framework. Not in the core scope now (D1). Source: https://crates.io/crates/teloxide
- Use the newest stable version of each crate at implementation time.

### 7.7 Home Assistant MQTT discovery

- Discovery topic: `<discovery_prefix>/<component>/[<node_id>/]<object_id>/config`. The default prefix is `homeassistant`.
- Device discovery: one message on `homeassistant/device/<object_id>/config` with `dev` (device), `o` (origin) and `cmps` (components). Each component has `p` (platform, for example `sensor`) and `unique_id`.
- HA publishes `online` on `homeassistant/status` when it starts. The bridge must then publish the discovery messages again. Retained discovery messages are also possible.
- `expire_after` makes a sensor "unavailable" when no new value arrives.
- An empty retained payload on the discovery topic removes the device.
- Source: https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery

## 8. Bridge configuration (proposal)

File `/etc/dog-house-bridge/config.toml` (example in `receiver/config.example.toml`):

```toml
[radio]
spi_device = "/dev/spidev0.0"
gpio_chip = "/dev/gpiochip0"
ce_gpio = 25            # BCM GPIO25, physical pin 22
channel = 76            # 0..=83 only (2400-2483 MHz)
data_rate = "250kbps"
address = "DHSN1"       # 5 bytes, same as the node

[mqtt]
host = "nvr.local"      # the NVR host name or IP address
port = 1883
username = "dog-house-bridge"
password_file = "/etc/dog-house-bridge/mqtt-password"   # never in git
discovery_prefix = "homeassistant"
base_topic = "dog-house-sensor"

[nodes.1]
name = "Dog house"
expire_after_s = 2700   # 3 missed readings
```

## 9. Home NVR setup (planned, needs approval)

Facts (2026-11-21): HA already runs on the home NVR (Debian 11) in the container `homeassistant` (`ghcr.io/home-assistant/home-assistant:stable`, host network, port 8123). There is no MQTT broker. Port 1883 is free.

Plan:

1. Add a Mosquitto container (`eclipse-mosquitto`, newest stable 2.x) on the NVR. Port 1883 on the LAN only. Anonymous access off. One user for the bridge and one user for HA. Persistence on.
2. Add the MQTT integration in HA. Point it to the broker on `localhost:1883` with the HA user.
3. Keep the passwords out of the repository. The repository contains only an example `mosquitto.conf` and a setup document.

CAUTION: Do not change the NVR before Tomasz approves it. Do not change other services or ports on the NVR. The NVR has limited free memory. Mosquitto needs only some MB.

## 10. Wiring (proposal, the spike confirms)

D1 mini to nRF24L01+:

| nRF24L01+ | D1 mini |
|-----------|---------|
| VCC | 3V3 (with a capacitor at the module) |
| GND | G |
| CE | D2 (GPIO4) |
| CSN | D8 (GPIO15) |
| SCK | D5 (GPIO14) |
| MOSI | D7 (GPIO13) |
| MISO | D6 (GPIO12) |
| IRQ | not connected |

D1 mini to AM2302: pin 1 VDD to a switched 3.3 V supply (GPIO or transistor, the spike selects), pin 2 DATA to D1 (GPIO5) with 10 kOhm to VDD, pin 3 not connected, pin 4 GND.

Deep sleep: D0 (GPIO16) to RST. Disconnect this wire when you flash the board if the upload fails.

Pi to nRF24L01+: VCC to pin 17 (3.3 V), GND to pin 20, CE to pin 22 (GPIO25), CSN to pin 24 (CE0), SCK to pin 23, MOSI to pin 19, MISO to pin 21.

## 11. Repository layout (planned)

```
docs/PRD.md                     this document
docs/decisions/                 decision records (node language)
docs/home-assistant/            example automations and dashboard
docs/nvr-mosquitto.md           broker setup (after approval)
protocol/                       no_std packet format crate + golden vectors
receiver/                       Rust bridge for the Raspberry Pi
node/                           node firmware (Rust or C++, from the spike)
```

## 12. Quality gates

- CI on GitHub Actions for each pull request: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and a cross-build of the bridge for `aarch64-unknown-linux-gnu`.
- Node CI: the Rust build with a pinned esp-rs toolchain, or `pio run` and `pio test -e native` for C++.
- Squash merge only when all checks pass. Branch protection on `main`.
- No secrets in the repository.

## 13. Open questions for Tomasz

1. Do you approve the Mosquitto container on the NVR (section 9)? Nothing changes on the NVR before your yes.
2. Which Pi do you use: Pi 3 or Pi 4? Which OS (Raspberry Pi OS 64-bit is recommended)?
3. Do you have a second nRF24L01+ module? (The design needs two.)
4. What is the distance from the dog house to the Pi, and how many walls are between them?
5. Do you accept the default limits: 0 °C, 30 °C, low battery 3.5 V, no data for 45 minutes?
6. A low-loss regulator (for example MCP1700) and an 18650 holder can be necessary. Do you have them? Any purchase needs your yes.
7. Do you want HA notifications on the HA mobile app, on Telegram, or both (optional item)?

## 14. Risks

| Risk | Effect | Action |
|------|--------|--------|
| Rust on the ESP8266 does not work | Node in C++ | Spike first; C++ fallback |
| D1 mini clone uses mA in deep sleep | Battery life of days | Measure first; cut CH340 power or use bare ESP-12 |
| Range too short through walls | Lost packets | 250 kbps, antenna placement, range test; PA+LNA module only after a power check |
| Cold weather | Less battery capacity, sensor fails below 3.3 V | Low battery alarm at 3.5 V |
| 2.4 GHz interference | Lost packets | Configurable channel, auto-retransmit |
