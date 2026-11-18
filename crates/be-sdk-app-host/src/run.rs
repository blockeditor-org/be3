//! One run of an app as its host keeps it: what the host told the app and heard from it since
//! its module last started, which a restart forgets (guides/apps/runtime.md, When an app stops).
//! A host keeps one `Run` per app and drives its pieces; `restart` is the one place that says
//! what a restart keeps, resends and drops.
use be_sdk_api::{Fonts, Preferences, ScreenRequest};

use crate::{
    accessibility::Graft,
    calls::{AppCall, Outbound},
    docks::Docks,
    holds::{Holds, Presents, Shown},
    pacing::Pacing,
    session::Session,
};

/// An app's run, with `A` the answers the host holds for a later poll (`Holds`).
pub struct Run<A> {
    pub session: Session,
    /// What the host has to send the app once its session runs.
    pub queued: Vec<Outbound>,
    pub holds: Holds<A>,
    pub shown: Shown,
    pub presents: Presents,
    pub pacing: Pacing,
    /// The screens last sent (`AppCall::Screens`).
    pub screens: Vec<ScreenRequest>,
    /// The host asked for the app's paintings and has yet to hear them.
    pub painting: bool,
    told: Told,
}

/// What the host told the run that it tells every run: each is sent again only when it changes.
struct Told {
    fonts: bool,
    describing: bool,
    preferences: Preferences,
    utc_offset: Option<i32>,
}

impl Told {
    fn new(preferences: Preferences) -> Self {
        Self {
            fonts: false,
            describing: false,
            preferences,
            utc_offset: None,
        }
    }
}

/// What the host tells its apps as it stands now, for `Run::tell`.
pub struct Telling<F> {
    /// The fonts the host bundles, made only when the run has yet to hear them.
    pub fonts: F,
    /// Whether the host asks apps to describe their views (`AppCall::Describe`).
    pub describing: bool,
    pub preferences: Preferences,
    pub utc_offset: i32,
}

/// What a host keeps of what an app's views described, outside the `Run`: a graft for each
/// screen and the actions each view offers.
pub trait Descriptions {
    /// Calls `each` with the graft of every screen of the app's views.
    fn grafts(&mut self, each: &mut dyn FnMut(&mut Graft));
    /// Forgets the actions every view described.
    fn forget_actions(&mut self);
}

impl<A> Run<A> {
    /// A first run, told `preferences` with its hello (`preferences`).
    pub fn new(session: Session, holds_textures: bool, preferences: Preferences) -> Self {
        Self {
            session,
            queued: Vec::new(),
            holds: Holds::default(),
            shown: Shown::new(holds_textures),
            presents: Presents::default(),
            pacing: Pacing::default(),
            screens: Vec::new(),
            painting: false,
            told: Told::new(preferences),
        }
    }

    /// The preferences the app was last told, which the host's answer to its hello carries.
    pub fn preferences(&self) -> Preferences {
        self.told.preferences
    }

    /// What of `telling` the app has yet to be told: the fonts once, then whether to describe,
    /// the preferences and the UTC offset whenever they change. All but the preferences wait
    /// for the session to run; the preferences a starting app is told with its hello.
    pub fn tell(&mut self, telling: Telling<impl FnOnce() -> Fonts>) -> Vec<AppCall> {
        let running = self.session.running();
        let told = &mut self.told;
        let mut calls = Vec::new();
        if running && !told.fonts {
            told.fonts = true;
            calls.push(AppCall::Fonts((telling.fonts)()));
        }
        if running && told.describing != telling.describing {
            told.describing = telling.describing;
            calls.push(AppCall::Describe(telling.describing));
        }
        if told.preferences != telling.preferences {
            told.preferences = telling.preferences;
            calls.push(AppCall::Preferences(telling.preferences));
        }
        if running && told.utc_offset != Some(telling.utc_offset) {
            told.utc_offset = Some(telling.utc_offset);
            calls.push(AppCall::UtcOffset(telling.utc_offset));
        }
        calls
    }

    /// Starts a new run of `app` in `session`, told `preferences` with its hello. What the old
    /// run was sent or held goes: its session, the calls queued for it, the answers held and
    /// what they wait for, the views it closed or left out (`Shown`), its presents, its frames
    /// in flight, the screens and paintings it was asked for, and what its instances' docks
    /// declared. Its views' accessibility panes stay, empty, and offer no actions until the new
    /// run describes them. It is told the fonts, whether to describe and the UTC offset again
    /// once it runs (`tell`); the host sends its instances' content again itself.
    pub fn restart(
        &mut self,
        session: Session,
        preferences: Preferences,
        app: &str,
        docks: &mut Docks,
        described: &mut impl Descriptions,
    ) {
        self.session = session;
        self.queued.clear();
        self.holds.clear();
        self.shown.clear();
        self.presents = Presents::default();
        self.pacing.reset();
        self.screens.clear();
        self.painting = false;
        self.told = Told::new(preferences);
        docks.restarted(app);
        described.grafts(&mut Graft::restarted);
        described.forget_actions();
    }
}

#[cfg(test)]
mod tests;
