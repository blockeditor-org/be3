use std::sync::{Arc, Mutex};

use beui_types::{PreferenceLayer, ThemeKind};
use winit::window::Theme;

#[derive(Default)]
pub(crate) struct SystemPreferences {
    window: Option<ThemeKind>,
    desktop: Arc<Mutex<PreferenceLayer>>,
}

impl SystemPreferences {
    pub(crate) fn window_theme(&mut self, theme: Option<Theme>) {
        self.window = theme.map(|theme| match theme {
            Theme::Dark => ThemeKind::Dark,
            Theme::Light => ThemeKind::Light,
        });
    }

    pub(crate) fn layer(&self) -> PreferenceLayer {
        let desktop = *self
            .desktop
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        desktop.over(PreferenceLayer {
            theme: self.window,
            motion: None,
            canvas_touch: None,
        })
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn watch_desktop(&self, wake: impl Fn() + Send + 'static) {
        let desktop = Arc::clone(&self.desktop);
        let started = std::thread::Builder::new()
            .name("beui-preferences".to_owned())
            .spawn(move || {
                if let Err(error) = pollster::block_on(portal::watch(&desktop, &wake)) {
                    eprintln!("beui: the desktop's appearance settings cannot be read: {error}");
                }
            });
        if let Err(error) = started {
            eprintln!("beui: the desktop's appearance settings are not watched: {error}");
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn watch_desktop(&self, _wake: impl Fn() + Send + 'static) {}
}

#[cfg(target_os = "linux")]
mod portal {
    use std::sync::Mutex;

    use ashpd::desktop::settings::Settings;
    use ashpd::zvariant::OwnedValue;
    use beui_types::{Motion, PreferenceLayer, ThemeKind};
    use futures_util::StreamExt;

    const APPEARANCE: &str = "org.freedesktop.appearance";
    const COLOR_SCHEME: &str = "color-scheme";
    const REDUCED_MOTION: &str = "reduced-motion";

    pub(super) async fn watch(
        desktop: &Mutex<PreferenceLayer>,
        wake: &impl Fn(),
    ) -> Result<(), ashpd::Error> {
        let settings = Settings::new().await?;
        let changes = settings.receive_setting_changed().await?;
        let mut layer = PreferenceLayer {
            theme: theme(settings.read::<u32>(APPEARANCE, COLOR_SCHEME).await.ok()),
            motion: motion(settings.read::<u32>(APPEARANCE, REDUCED_MOTION).await.ok()),
            canvas_touch: None,
        };
        publish(desktop, layer, wake);
        futures_util::pin_mut!(changes);
        while let Some(setting) = changes.next().await {
            if setting.namespace() != APPEARANCE {
                continue;
            }
            let value = number(setting.value());
            match setting.key() {
                COLOR_SCHEME => layer.theme = theme(value),
                REDUCED_MOTION => layer.motion = motion(value),
                _ => continue,
            }
            publish(desktop, layer, wake);
        }
        Ok(())
    }

    fn publish(desktop: &Mutex<PreferenceLayer>, layer: PreferenceLayer, wake: &impl Fn()) {
        *desktop
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = layer;
        wake();
    }

    fn number(value: &OwnedValue) -> Option<u32> {
        value.downcast_ref::<u32>().ok()
    }

    fn theme(scheme: Option<u32>) -> Option<ThemeKind> {
        match scheme? {
            1 => Some(ThemeKind::Dark),
            2 => Some(ThemeKind::Light),
            _ => None,
        }
    }

    fn motion(reduced: Option<u32>) -> Option<Motion> {
        match reduced? {
            1 => Some(Motion::Instant),
            _ => None,
        }
    }
}
