#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Wanted {
    pub(super) app: String,
    pub(super) module: String,
    pub(super) config: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Step {
    Nothing,
    Stop,
    Configure(Vec<u8>),
    Fetch { app: String, module: String },
}

#[derive(Default)]
pub(super) struct Loader {
    loaded: Option<(String, String)>,
    fetching: Option<(String, String)>,
    wanted: Option<Wanted>,
}

impl Loader {
    pub(super) fn installed(&mut self, wanted: Wanted) {
        self.loaded = Some((wanted.app.clone(), wanted.module.clone()));
        self.fetching = None;
        self.wanted = Some(wanted);
    }

    pub(super) fn want(&mut self, wanted: Option<Wanted>) -> Step {
        if self.wanted == wanted {
            return Step::Nothing;
        }
        self.wanted.clone_from(&wanted);
        let Some(wanted) = wanted else {
            self.loaded = None;
            self.fetching = None;
            return Step::Stop;
        };
        let source = (wanted.app.clone(), wanted.module.clone());
        if self.loaded.as_ref() == Some(&source) {
            return Step::Configure(wanted.config);
        }
        if self.fetching.as_ref() == Some(&source) {
            return Step::Nothing;
        }
        self.fetching = Some(source);
        Step::Fetch {
            app: wanted.app,
            module: wanted.module,
        }
    }

    pub(super) fn fetched(&mut self, app: &str, module: &str) -> Option<Wanted> {
        let wanted = self.wanted.clone()?;
        if wanted.app != app || wanted.module != module {
            return None;
        }
        self.installed(wanted.clone());
        Some(wanted)
    }

    /// Whether this module is on its way.
    pub(super) fn fetching(&self, app: &str, module: &str) -> bool {
        self.fetching
            .as_ref()
            .is_some_and(|(fetching_app, fetching_module)| {
                fetching_app == app && fetching_module == module
            })
    }

    /// The module could not be fetched or loaded: it is neither running nor on its way, so
    /// the next change to the remaps fetches it again instead of reconfiguring nothing.
    pub(super) fn failed(&mut self) {
        self.loaded = None;
        self.fetching = None;
    }

    pub(super) fn wanted(&self) -> Option<&Wanted> {
        self.wanted.as_ref()
    }
}

#[cfg(test)]
mod tests;
