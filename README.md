# dog-house-sensor

A battery sensor for a dog house. It measures the temperature and the humidity and sends them to Home Assistant.

There is no Wi-Fi at the dog house. A Wemos D1 mini with an AM2302 (DHT22) sensor sends the values on a 2.4 GHz nRF24L01+ radio link to a Raspberry Pi in the house. A Rust bridge program on the Pi publishes the values to an MQTT broker. Home Assistant finds the sensors through MQTT discovery and sends the alarms.

```
D1 mini + AM2302 + nRF24L01+  --2.4 GHz-->  Raspberry Pi (Rust bridge)  --MQTT-->  Mosquitto + Home Assistant
```

## Status

Planning. There is no code yet. The plan is in the GitHub issues.

## Documents

- [Product Requirements Document](docs/PRD.md): decisions, requirements, research, battery estimate, open questions.

## Rules

- Language: ASD-STE100 Simplified Technical English.
- Rust wherever possible. The node language comes from the spike (see the PRD, section 7.3).
- Never commit secrets (MQTT passwords, tokens). Use the example files and keep real values outside the repository.
- Radio: the defaults are legal (nRF24 channels 0 to 83, 2400 to 2483 MHz, max 10 mW e.i.r.p.). Other channels and power levels need `allow_out_of_band = true` and are not legal in Poland and the EU (research use only, see PRD 7.1a).

## License

To be decided.
