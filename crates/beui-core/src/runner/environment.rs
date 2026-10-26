use crate::app::automation::{MAX_PIXELS, WindowSize};
use crate::geometry::Vec2;

pub const PLATFORM: &str = "BEUI_PLATFORM";
pub const LAYERS: &str = "BEUI_LAYERS";
pub const RENDERER: &str = "BEUI_RENDERER";
pub const HEADLESS_WINDOW: &str = "BEUI_HEADLESS_WINDOW";
pub const HEADLESS_SCREEN: &str = "BEUI_HEADLESS_SCREEN";
pub const AUTOMATION: &str = "BEUI_AUTOMATION";
pub const AUTOMATION_LISTENER: &str = "BEUI_AUTOMATION_LISTENER";

pub const VARIABLES: &[&str] = &[
    PLATFORM,
    LAYERS,
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
    pub layers: Vec<Stacked>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stacked {
    Space,
}

impl Stacked {
    pub const ALL: &[Self] = &[Self::Space];

    pub fn name(self) -> &'static str {
        match self {
            Self::Space => "space",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|layer| layer.name() == name)
    }
}

pub fn choose(native: &str, var: impl Fn(&str) -> Option<String>) -> Result<Choice, String> {
    let headless = match var(PLATFORM).as_deref() {
        None => false,
        Some(name) if name == native => false,
        Some("headless") => true,
        Some(other) => {
            let hint = match Stacked::named(other) {
                Some(layer) => format!("; {} is a layer: {LAYERS}={}", layer.name(), layer.name()),
                None => String::new(),
            };
            return Err(format!(
                "{PLATFORM} takes {}, not {other}{hint}",
                list(&[native, "headless"])
            ));
        }
    };
    let layers = match var(LAYERS) {
        None => Vec::new(),
        Some(names) => names
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| {
                Stacked::named(name).ok_or_else(|| {
                    let known: Vec<&str> = Stacked::ALL.iter().map(|layer| layer.name()).collect();
                    format!(
                        "{LAYERS} takes a comma-separated list of {}, bottom first, not {name}",
                        list(&known)
                    )
                })
            })
            .collect::<Result<_, _>>()?,
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
                "{RENDERER}=none needs the headless platform: {PLATFORM}=headless"
            ));
        }
        Some(other) => {
            return Err(format!("{RENDERER} takes wgpu or none, not {other}"));
        }
    };
    Ok(Choice {
        platform,
        layers,
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
