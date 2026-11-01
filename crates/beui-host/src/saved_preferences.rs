use std::path::{Path, PathBuf};

use beui::PreferenceLayer;
use beui::PreferenceSource;
use beui::Rect;
use beui_core::app::{App, FrameTimes, Lifecycle, Setup};
use beui_core::context::Context;

pub(crate) struct SavedPreferences<A> {
    app: A,
    path: PathBuf,
    saved: Option<PreferenceLayer>,
}

impl<A: App> SavedPreferences<A> {
    pub(crate) fn new(app: A, path: PathBuf) -> Self {
        Self {
            app,
            path,
            saved: None,
        }
    }
}

impl<A: App> App for SavedPreferences<A> {
    fn update(&mut self, context: &Context, rect: Rect) {
        if self.saved.is_none() {
            let loaded = load(&self.path);
            context.set_preference_layer(PreferenceSource::App, loaded);
            self.saved = Some(loaded);
        }
        self.app.update(context, rect);
        let chosen = context.preference_layer(PreferenceSource::App);
        if self.saved != Some(chosen) {
            self.saved = Some(chosen);
            if let Err(error) = save(&self.path, chosen) {
                eprintln!(
                    "beui: the preferences could not be saved to {}: {error}",
                    self.path.display()
                );
            }
        }
    }

    fn setup(&mut self, setup: &Setup) {
        self.app.setup(setup);
    }

    fn renderer_replaced(&mut self, setup: &Setup) {
        self.app.renderer_replaced(setup);
    }

    fn close_requested(&mut self) -> bool {
        self.app.close_requested()
    }

    fn exiting(&mut self) {
        self.app.exiting();
    }

    fn lifecycle(&mut self, change: Lifecycle) {
        self.app.lifecycle(change);
    }

    fn busy(&self) -> Vec<String> {
        self.app.busy()
    }

    fn environment(&self) -> Vec<(String, String)> {
        self.app.environment()
    }

    fn drawing(&self) -> bool {
        self.app.drawing()
    }

    fn request_painting(&mut self) {
        self.app.request_painting();
    }

    fn other_frames(&mut self) -> Vec<(String, FrameTimes)> {
        self.app.other_frames()
    }
}

fn load(path: &Path) -> PreferenceLayer {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(path: &Path, preferences: PreferenceLayer) -> std::io::Result<()> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let json = serde_json::to_vec_pretty(&preferences).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}
