use super::*;

mod a_monitor_is_known_by_its_edid_or_else_its_connector;
mod a_saved_mode_wins_while_the_monitor_offers_it;
mod refresh_rates_keep_their_fractions;
mod the_default_is_the_preferred_size_at_its_fastest_refresh;

fn mode(width: u32, height: u32, refresh_millihertz: u32) -> DisplayMode {
    DisplayMode {
        width,
        height,
        refresh_millihertz,
    }
}

fn candidate(width: u32, height: u32, refresh_millihertz: u32) -> Candidate {
    Candidate {
        mode: mode(width, height, refresh_millihertz),
        preferred: false,
        interlaced: false,
    }
}

fn preferred(width: u32, height: u32, refresh_millihertz: u32) -> Candidate {
    Candidate {
        preferred: true,
        ..candidate(width, height, refresh_millihertz)
    }
}

fn gaming_monitor() -> Vec<Candidate> {
    vec![
        preferred(2560, 1440, 59_951),
        candidate(2560, 1440, 239_970),
        candidate(2560, 1440, 143_998),
        candidate(3840, 2160, 30_000),
        candidate(1920, 1080, 60_000),
        candidate(1920, 1080, 60_000),
        Candidate {
            interlaced: true,
            ..candidate(1920, 1080, 120_000)
        },
    ]
}
