use std::path::{Path, PathBuf};

use beui::PreferenceLayer;
use beui::PreferenceSource;
use beui::Rect;
use beui_core::context::Context;
use beui_core::program::{FrameTimes, Lifecycle, Program, Setup};

pub(crate) struct SavedPreferences<A> {
    program: A,
    path: PathBuf,
    saved: Option<PreferenceLayer>,
}

impl<A: Program> SavedPreferences<A> {
    pub(crate) fn new(program: A, path: PathBuf) -> Self {
        Self {
            program,
            path,
            saved: None,
        }
    }
}

impl<A: Program> Program for SavedPreferences<A> {
    fn update(&mut self, context: &Context, rect: Rect) {
        if self.saved.is_none() {
            let loaded = load(&self.path);
            context.set_preference_layer(PreferenceSource::Program, loaded);
            self.saved = Some(loaded);
        }
        self.program.update(context, rect);
        let chosen = context.preference_layer(PreferenceSource::Program);
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
        self.program.setup(setup);
    }

    fn renderer_replaced(&mut self, setup: &Setup) {
        self.program.renderer_replaced(setup);
    }

    fn close_requested(&mut self) -> bool {
        self.program.close_requested()
    }

    fn exiting(&mut self) {
        self.program.exiting();
    }

    fn lifecycle(&mut self, change: Lifecycle) {
        self.program.lifecycle(change);
    }

    fn busy(&self) -> Vec<String> {
        self.program.busy()
    }

    fn out_of_step(&self) -> Option<Vec<String>> {
        self.program.out_of_step()
    }

    fn connected(&self) -> Option<bool> {
        self.program.connected()
    }

    fn in_step_due(&self) -> Option<std::time::Instant> {
        self.program.in_step_due()
    }

    fn environment(&self) -> Vec<(String, String)> {
        self.program.environment()
    }

    fn drawing(&self) -> bool {
        self.program.drawing()
    }

    fn request_painting(&mut self) {
        self.program.request_painting();
    }

    fn other_frames(&mut self) -> Vec<(String, FrameTimes)> {
        self.program.other_frames()
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
