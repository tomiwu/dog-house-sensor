//! Legal check for the radio settings (PRD section 7.1a).
//!
//! The node and the bridge call [`check_radio`] with the same inputs.

/// Highest channel that is inside the permitted band (2483.5 MHz).
pub const MAX_LEGAL_CHANNEL: u8 = 83;
/// Highest e.i.r.p. in dBm (10 mW).
pub const MAX_EIRP_DBM: f32 = 10.0;

/// Radio settings to check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadioSettings {
    /// nRF24L01+ channel. The frequency is 2400 MHz + channel.
    pub channel: u8,
    /// nRF24L01+ PA level in dBm.
    pub pa_level_dbm: f32,
    /// Gain of an external PA in dB. 0 for a module without PA.
    pub pa_gain_db: f32,
    /// Antenna gain in dBi.
    pub antenna_gain_dbi: f32,
}

impl RadioSettings {
    /// e.i.r.p. in dBm: PA level + PA gain + antenna gain.
    pub fn eirp_dbm(&self) -> f32 {
        self.pa_level_dbm + self.pa_gain_db + self.antenna_gain_dbi
    }
}

/// One broken rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Violation {
    /// The channel is above [`MAX_LEGAL_CHANNEL`].
    ChannelAboveLimit { channel: u8 },
    /// The e.i.r.p. is above [`MAX_EIRP_DBM`], or it is not a number.
    EirpAboveLimit { eirp_dbm: f32 },
}

/// A list of at most two violations (no allocation). The channel violation
/// comes first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Violations {
    items: [Option<Violation>; 2],
}

impl Violations {
    /// Number of violations.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// True if the list has no violation.
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Iterate over the violations.
    pub fn iter(&self) -> impl Iterator<Item = Violation> + '_ {
        self.items.iter().flatten().copied()
    }
}

/// Result of the legal check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadioCheck {
    Legal,
    Violations(Violations),
}

/// Check the channel and the e.i.r.p. against the legal limits.
pub fn check_radio(settings: &RadioSettings) -> RadioCheck {
    let channel = (settings.channel > MAX_LEGAL_CHANNEL).then_some(Violation::ChannelAboveLimit {
        channel: settings.channel,
    });
    let eirp_dbm = settings.eirp_dbm();
    let eirp = (eirp_dbm.is_nan() || eirp_dbm > MAX_EIRP_DBM)
        .then_some(Violation::EirpAboveLimit { eirp_dbm });
    let violations = Violations {
        items: [channel, eirp],
    };
    if violations.is_empty() {
        RadioCheck::Legal
    } else {
        RadioCheck::Violations(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(
        channel: u8,
        pa_level_dbm: f32,
        pa_gain_db: f32,
        antenna_gain_dbi: f32,
    ) -> RadioSettings {
        RadioSettings {
            channel,
            pa_level_dbm,
            pa_gain_db,
            antenna_gain_dbi,
        }
    }

    fn violations(settings: RadioSettings) -> Violations {
        match check_radio(&settings) {
            RadioCheck::Legal => panic!("expected violations"),
            RadioCheck::Violations(list) => list,
        }
    }

    fn assert_violations(settings: RadioSettings, expected: &[Violation]) {
        let list = violations(settings);
        assert_eq!(list.len(), expected.len());
        assert!(list.iter().eq(expected.iter().copied()), "{list:?}");
    }

    #[test]
    fn default_settings_are_legal() {
        assert_eq!(check_radio(&settings(76, 0.0, 0.0, 2.0)), RadioCheck::Legal);
    }

    #[test]
    fn channel_limits() {
        assert_eq!(check_radio(&settings(0, 0.0, 0.0, 2.0)), RadioCheck::Legal);
        assert_eq!(check_radio(&settings(83, 0.0, 0.0, 2.0)), RadioCheck::Legal);
        assert_violations(
            settings(84, 0.0, 0.0, 2.0),
            &[Violation::ChannelAboveLimit { channel: 84 }],
        );
    }

    #[test]
    fn channel_108_is_a_violation() {
        assert_violations(
            settings(108, 0.0, 0.0, 2.0),
            &[Violation::ChannelAboveLimit { channel: 108 }],
        );
    }

    #[test]
    fn pa_gain_can_break_the_eirp_limit() {
        assert_violations(
            settings(76, 0.0, 20.0, 2.0),
            &[Violation::EirpAboveLimit { eirp_dbm: 22.0 }],
        );
    }

    #[test]
    fn exactly_10_dbm_is_legal() {
        assert_eq!(check_radio(&settings(76, 0.0, 8.0, 2.0)), RadioCheck::Legal);
    }

    #[test]
    fn both_violations_are_listed_channel_first() {
        assert_violations(
            settings(108, 0.0, 20.0, 2.0),
            &[
                Violation::ChannelAboveLimit { channel: 108 },
                Violation::EirpAboveLimit { eirp_dbm: 22.0 },
            ],
        );
    }

    #[test]
    fn not_a_number_is_a_violation() {
        let list = violations(settings(76, f32::NAN, 0.0, 2.0));
        assert_eq!(list.len(), 1);
        assert!(
            list.iter()
                .all(|v| matches!(v, Violation::EirpAboveLimit { eirp_dbm } if eirp_dbm.is_nan()))
        );
    }
}
