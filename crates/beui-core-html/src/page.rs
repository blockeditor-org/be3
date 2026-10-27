use beui_types::Vec2;
use beui_types::{Motion, PreferenceLayer, Preferences, ThemeKind};
use reactive::{Get, Memo, ReadSignal, Signal, create_memo};

#[derive(Clone, Copy)]
struct PageSignals {
    theme: Signal<Option<ThemeKind>>,
    motion: Signal<Option<Motion>>,
    viewport: Signal<Vec2>,
    pixels_per_point: Signal<f32>,
    font_revision: Signal<u64>,
}

thread_local! {
    static SIGNALS: PageSignals = PageSignals {
        theme: crate::global_signal(None),
        motion: crate::global_signal(None),
        viewport: crate::global_signal(Vec2::ZERO),
        pixels_per_point: crate::global_signal(1.0),
        font_revision: crate::global_signal(0),
    };
}

thread_local! {
    static TOUCHED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn note_pointer(touch: bool) {
    TOUCHED.with(|touched| touched.set(touch));
}

pub fn touched_by_finger() -> bool {
    TOUCHED.with(std::cell::Cell::get)
}

fn signals() -> PageSignals {
    SIGNALS.with(|signals| *signals)
}

pub(crate) fn set_theme(theme: ThemeKind) {
    use reactive::Set;

    signals().theme.set(Some(theme));
}

pub fn use_preferences() -> Memo<Preferences> {
    let signals = signals();
    create_memo(move || {
        PreferenceLayer {
            theme: signals.theme.get(),
            motion: signals.motion.get(),
            canvas_touch: None,
        }
        .resolve()
    })
}

pub fn motion() -> Motion {
    let layer = PreferenceLayer {
        theme: None,
        motion: signals().motion.get_untracked(),
        canvas_touch: None,
    };
    layer.resolve().motion
}

pub fn watch_motion() -> Memo<Motion> {
    let preferences = use_preferences();
    create_memo(move || preferences.get().motion)
}

pub fn use_viewport_size() -> ReadSignal<Vec2> {
    signals().viewport.read_only()
}

pub fn pixels_per_point() -> ReadSignal<f32> {
    signals().pixels_per_point.read_only()
}

pub fn use_font_revision() -> ReadSignal<u64> {
    signals().font_revision.read_only()
}

pub fn copy_text(text: String) {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = crate::dom::window() {
        let _ = window.navigator().clipboard().write_text(&text);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn watch(window: &web_sys::Window) -> Vec<crate::dom::Listener> {
    use reactive::Set;

    let signals = signals();
    let mut listeners = Vec::new();
    let report = {
        let window = window.clone();
        move || {
            let query = |media: &str| {
                window
                    .match_media(media)
                    .ok()
                    .flatten()
                    .is_some_and(|list| list.matches())
            };
            let theme = match (
                query("(prefers-color-scheme: dark)"),
                query("(prefers-color-scheme: light)"),
            ) {
                (true, _) => Some(ThemeKind::Dark),
                (false, true) => Some(ThemeKind::Light),
                (false, false) => None,
            };
            let motion = query("(prefers-reduced-motion: reduce)").then_some(Motion::Instant);
            let size = Vec2::new(
                window
                    .inner_width()
                    .ok()
                    .and_then(|width| width.as_f64())
                    .unwrap_or_default() as f32,
                window
                    .inner_height()
                    .ok()
                    .and_then(|height| height.as_f64())
                    .unwrap_or_default() as f32,
            );
            let scale = window.device_pixel_ratio() as f32;
            if signals.theme.get_untracked() != theme {
                signals.theme.set(theme);
            }
            if signals.motion.get_untracked() != motion {
                signals.motion.set(motion);
            }
            if signals.viewport.get_untracked() != size {
                signals.viewport.set(size);
            }
            if signals.pixels_per_point.get_untracked() != scale {
                signals.pixels_per_point.set(scale);
            }
            crate::frame::request_frame();
        }
    };
    report();
    for media in [
        "(prefers-color-scheme: dark)",
        "(prefers-color-scheme: light)",
        "(prefers-reduced-motion: reduce)",
    ] {
        if let Some(list) = window.match_media(media).ok().flatten() {
            let report = report.clone();
            listeners.push(crate::dom::listen(
                list.as_ref(),
                "change",
                false,
                move |_: web_sys::Event| report(),
            ));
        }
    }
    let resized = report.clone();
    listeners.push(crate::dom::listen(
        window.as_ref(),
        "resize",
        false,
        move |_: web_sys::Event| resized(),
    ));
    if let Some(fonts) = window.document().map(|document| document.fonts()) {
        listeners.push(crate::dom::listen(
            fonts.as_ref(),
            "loadingdone",
            false,
            move |_: web_sys::Event| {
                signals.font_revision.update(|revision| *revision += 1);
                crate::frame::request_frame();
            },
        ));
    }
    listeners
}

pub fn set_title(title: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(document) = crate::dom::document() {
        document.set_title(title);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = title;
}

pub fn set_background(color: beui_types::Color32) {
    #[cfg(target_arch = "wasm32")]
    if let Some(document) = crate::dom::document() {
        let css = crate::css::color(color);
        if let Some(body) = document.body() {
            let _ = body.style().set_property("background-color", &css);
        }
        let meta = document
            .query_selector("meta[name=\"theme-color\"]")
            .ok()
            .flatten()
            .or_else(|| {
                let meta = document.create_element("meta").ok()?;
                meta.set_attribute("name", "theme-color").ok()?;
                document.head()?.append_child(&meta).ok()?;
                Some(meta)
            });
        if let Some(meta) = meta {
            let _ = meta.set_attribute("content", &css);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = color;
}
