use crate::app::automation::{MAX_PIXELS, WindowSize};
use crate::geometry::Vec2;

pub const ADAPTER: &str = "BEUI_ADAPTER";
pub const SPACE_ADAPTER: &str = "BEUI_SPACE_ADAPTER";
pub const RENDERER: &str = "BEUI_RENDERER";
pub const HEADLESS_WINDOW: &str = "BEUI_HEADLESS_WINDOW";
pub const HEADLESS_SCREEN: &str = "BEUI_HEADLESS_SCREEN";
pub const AUTOMATION: &str = "BEUI_AUTOMATION";
pub const XR: &str = "BEUI_XR";

pub const VARIABLES: &[&str] = &[
    ADAPTER,
    SPACE_ADAPTER,
    RENDERER,
    HEADLESS_WINDOW,
    HEADLESS_SCREEN,
    AUTOMATION,
    XR,
];

pub const HEADLESS_WINDOW_SIZE: &str = "1100x720";

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub adapter: Base,
    pub space: bool,
    pub xr: bool,
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
    let adapter = var(ADAPTER);
    let space = adapter.as_deref() == Some("space");
    let base = match (adapter.as_deref(), var(SPACE_ADAPTER)) {
        (Some("space"), sub) => sub,
        (_, Some(_)) => {
            return Err(format!("{SPACE_ADAPTER} is for {ADAPTER}=space only"));
        }
        (adapter, None) => adapter.map(str::to_owned),
    };
    let headless = match base.as_deref() {
        None => false,
        Some(name) if name == native => false,
        Some("headless") => true,
        Some(other) => {
            let variable = if space { SPACE_ADAPTER } else { ADAPTER };
            return Err(format!(
                "{variable} takes {}, not {other}",
                list(&names(!space))
            ));
        }
    };
    let adapter = if headless {
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
                return Err(format!("{variable} is for the headless adapter only"));
            }
        }
        Base::Native
    };
    let render = match var(RENDERER).as_deref() {
        None | Some("wgpu") => true,
        Some("none") if headless => false,
        Some("none") => {
            return Err(format!(
                "{RENDERER}=none needs the headless adapter: {ADAPTER}=headless, or \
                 {SPACE_ADAPTER}=headless under {ADAPTER}=space"
            ));
        }
        Some(other) => {
            return Err(format!("{RENDERER} takes wgpu or none, not {other}"));
        }
    };
    let xr = match var(XR).as_deref() {
        None => false,
        Some("openxr") if space => true,
        Some("openxr") => return Err(format!("{XR} is for {ADAPTER}=space only")),
        Some(other) => return Err(format!("{XR} takes openxr, not {other}")),
    };
    Ok(Choice {
        adapter,
        space,
        xr,
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
