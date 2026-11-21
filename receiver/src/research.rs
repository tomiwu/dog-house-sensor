//! Research mode: the warning for settings outside the legal limits (PRD section 7.1a).

use std::time::Duration;

use protocol::{RadioCheck, RadioSettings, check_radio};

/// Text of the warning. The text is fixed by PRD requirement B7.
pub const OUT_OF_BAND_WARNING: &str = "Channel/power outside 2400-2483.5 MHz / 10 mW e.i.r.p. This is not legal in Poland and the EU. Research use only.";

/// Time between two warnings.
pub const WARNING_INTERVAL: Duration = Duration::from_secs(3600);

/// The warning for these settings, or `None` if no warning is needed.
///
/// A warning is needed when the settings are not legal. The configuration
/// check rejects such settings unless `allow_out_of_band` is true, so the
/// warning is for the accepted research settings.
pub fn out_of_band_warning(
    settings: &RadioSettings,
    allow_out_of_band: bool,
) -> Option<&'static str> {
    let illegal = matches!(check_radio(settings), RadioCheck::Violations(_));
    (allow_out_of_band && illegal).then_some(OUT_OF_BAND_WARNING)
}

/// Log the warning now and then each hour on a background thread.
pub fn spawn_hourly_warning(warning: &'static str) {
    eprintln!("WARNING: {warning}");
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(WARNING_INTERVAL);
            eprintln!("WARNING: {warning}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(channel: u8, pa_level_dbm: f32, pa_gain_db: f32) -> RadioSettings {
        RadioSettings {
            channel,
            pa_level_dbm,
            pa_gain_db,
            antenna_gain_dbi: 2.0,
        }
    }

    #[test]
    fn legal_settings_give_no_warning() {
        assert_eq!(out_of_band_warning(&settings(76, 0.0, 0.0), false), None);
        assert_eq!(out_of_band_warning(&settings(76, 0.0, 0.0), true), None);
    }

    #[test]
    fn out_of_band_channel_with_flag_gives_the_warning() {
        for channel in [84, 108] {
            assert_eq!(
                out_of_band_warning(&settings(channel, 0.0, 0.0), true),
                Some(OUT_OF_BAND_WARNING)
            );
        }
    }

    #[test]
    fn too_much_power_with_flag_gives_the_warning() {
        assert_eq!(
            out_of_band_warning(&settings(76, 0.0, 20.0), true),
            Some(OUT_OF_BAND_WARNING)
        );
    }

    #[test]
    fn warning_text_is_the_text_of_the_prd() {
        assert_eq!(
            OUT_OF_BAND_WARNING,
            "Channel/power outside 2400-2483.5 MHz / 10 mW e.i.r.p. This is not legal in Poland and the EU. Research use only."
        );
    }

    #[test]
    fn the_interval_is_one_hour() {
        assert_eq!(WARNING_INTERVAL.as_secs(), 3600);
    }
}
