use ui_types::{PreferenceLayer, Preferences};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreferenceSource {
    System,
    App,
    Developer,
}

impl PreferenceSource {
    pub const ALL: [Self; 3] = [Self::System, Self::App, Self::Developer];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreferenceLayers {
    system: PreferenceLayer,
    app: PreferenceLayer,
    developer: PreferenceLayer,
}

impl PreferenceLayers {
    pub fn layer(self, source: PreferenceSource) -> PreferenceLayer {
        match source {
            PreferenceSource::System => self.system,
            PreferenceSource::App => self.app,
            PreferenceSource::Developer => self.developer,
        }
    }

    pub fn layer_mut(&mut self, source: PreferenceSource) -> &mut PreferenceLayer {
        match source {
            PreferenceSource::System => &mut self.system,
            PreferenceSource::App => &mut self.app,
            PreferenceSource::Developer => &mut self.developer,
        }
    }

    pub fn below(self, source: PreferenceSource) -> PreferenceLayer {
        PreferenceSource::ALL
            .into_iter()
            .take_while(|lower| *lower != source)
            .fold(PreferenceLayer::EMPTY, |below, lower| {
                self.layer(lower).over(below)
            })
    }

    pub fn resolve(self) -> Preferences {
        self.developer.over(self.app).over(self.system).resolve()
    }
}
