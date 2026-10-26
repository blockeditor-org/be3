use crate::app::automation::{MAX_PIXELS, WindowSize};
use crate::geometry::Vec2;

pub const PLATFORM: &str = "BEUI_PLATFORM";
pub const SPACE_PLATFORM: &str = "BEUI_SPACE_PLATFORM";
pub const RENDERER: &str = "BEUI_RENDERER";
pub const HEADLESS_WINDOW: &str = "BEUI_HEADLESS_WINDOW";
pub const HEADLESS_SCREEN: &str = "BEUI_HEADLESS_SCREEN";
pub const AUTOMATION: &str = "BEUI_AUTOMATION";
pub const AUTOMATION_LISTENER: &str = "BEUI_AUTOMATION_LISTENER";

pub const VARIABLES: &[&str] = &[
    PLATFORM,
    SPACE_PLATFORM,
    RENDERER,
    HEADLESS_WINDOW,
    HEADLESS_SCREEN,
    AUTOMATION,
    AUTOMATION_LISTENER,
];

pub const HEADLESS_WINDOW_SIZE: &str = "1100x720";

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub platform: Base,
    pub space: bool,
    pub render: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Base {
    Native,
    Headless {
        window: WindowSize,
        screen: Option<Vec2>,
    },
}

pub fn choose(native: &str, var: impl Fn(&str) -> Option<String>) -> Result<Choice, String> {
    let names = |space: bool| {
        let mut names = vec![native, "headless"];
        if space {
            names.push("space");
        }
        names
    };
    let platform = var(PLATFORM);
    let space = platform.as_deref() == Some("space");
    let base = match (platform.as_deref(), var(SPACE_PLATFORM)) {
        (Some("space"), sub) => sub,
        (_, Some(_)) => {
            return Err(format!("{SPACE_PLATFORM} is for {PLATFORM}=space only"));
        }
        (platform, None) => platform.map(str::to_owned),
    };
    let headless = match base.as_deref() {
        None => false,
        Some(name) if name == native => false,
        Some("headless") => true,
        Some(other) => {
            let variable = if space { SPACE_PLATFORM } else { PLATFORM };
            return Err(format!(
                "{variable} takes {}, not {other}",
                list(&names(!space))
            ));
        }
    };
    let platform = if headless {
        let window = var(HEADLESS_WINDOW).unwrap_or_else(|| HEADLESS_WINDOW_SIZE.to_owned());
        let window = WindowSize::parse(&window)
            .ok_or_else(|| format!("{HEADLESS_WINDOW} takes WIDTHxHEIGHT[@SCALE], not {window}"))?;
        let screen = match var(HEADLESS_SCREEN) {
            Some(screen) => Some(
                WindowSize::parse(&screen)
                    .ok_or_else(|| format!("{HEADLESS_SCREEN} takes WIDTHxHEIGHT, not {screen}"))?
                    .size,
            ),
            None => None,
        };
        if let Some(size) = screen
            && !(WindowSize {
                size,
                scale: window.scale,
            })
            .fits()
        {
            return Err(format!(
                "{HEADLESS_SCREEN} at scale {} is more than {MAX_PIXELS} pixels across",
                window.scale,
            ));
        }
        Base::Headless { window, screen }
    } else {
        for variable in [HEADLESS_WINDOW, HEADLESS_SCREEN] {
            if var(variable).is_some() {
                return Err(format!("{variable} is for the headless platform only"));
            }
        }
        Base::Native
    };
    let render = match var(RENDERER).as_deref() {
        None | Some("wgpu") => true,
        Some("none") if headless => false,
        Some("none") => {
            return Err(format!(
                "{RENDERER}=none needs the headless platform: {PLATFORM}=headless, or \
                 {SPACE_PLATFORM}=headless under {PLATFORM}=space"
            ));
        }
        Some(other) => {
            return Err(format!("{RENDERER} takes wgpu or none, not {other}"));
        }
    };
    Ok(Choice {
        platform,
        space,
        render,
    })
}

fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests;
