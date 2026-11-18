//! One run of an app as its host keeps it: what the host told the app and heard from it since
//! its module last started, which a restart forgets (guides/apps/runtime.md, When an app stops).
//! A host keeps one `Run` per app and drives its pieces; `restart` is the one place that says
//! what a restart keeps, resends and drops.
use be_sdk_api::{Fonts, Preferences, ScreenId, ScreenLayout, ScreenPlacement, ScreenRequest};

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
    queued: Vec<Outbound>,
    pub holds: Holds<A>,
    pub shown: Shown,
    pub presents: Presents,
    pub pacing: Pacing,
    /// The screens last sent (`AppCall::Screens`).
    pub screens: Vec<ScreenRequest>,
    /// The app's screens as the host lays them out: its last layout, with the anchored views it
    /// closed kept until the frame that closed them is drawn (`place`, `lay_out`).
    layout: ScreenLayout,
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
/// screen and the actions each view offers. The library says only that a restart forgets a
/// view's actions, not how the host keeps them.
pub trait Descriptions {
    /// Calls `each` with the graft of every screen of the app's views.
    fn grafts(&mut self, each: &mut dyn FnMut(&mut Graft));
    /// Forgets the actions every view described.
    fn forget_actions(&mut self);
}

impl<A> Run<A> {
    /// A first run, told `preferences` with its hello (`Run::preferences`).
    pub fn new(session: Session, draws_presents: bool, preferences: Preferences) -> Self {
        Self {
            session,
            queued: Vec::new(),
            holds: Holds::default(),
            shown: Shown::new(draws_presents),
            presents: Presents::default(),
            pacing: Pacing::default(),
            screens: Vec::new(),
            layout: ScreenLayout::default(),
            painting: false,
            told: Told::new(preferences),
        }
    }

    /// The app's screens as the host lays them out.
    pub fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    /// Takes the app's new screens: answers what to lay out now (`lay_out`). An anchored view
    /// (`anchored`) they leave out stays, showing its last frame, until the frame that closed it
    /// is drawn (`Shown::place`).
    pub fn place(
        &mut self,
        layout: ScreenLayout,
        anchored: impl Fn(ScreenId) -> bool,
    ) -> ScreenLayout {
        self.shown.place(&self.layout, layout, anchored)
    }

    /// Lays the app's screens out as `layout`, keeping the last frame of each view it leaves out
    /// that the host still holds (`holds`; `Shown::keep_left_out`).
    pub fn lay_out(&mut self, layout: ScreenLayout, holds: impl Fn(&ScreenPlacement) -> bool) {
        self.shown.keep_left_out(&self.layout, &layout, holds);
        self.layout = layout;
    }

    /// The placement a screen is drawn from: its kept last frame while it has one, else the
    /// layout's.
    pub fn placement(&self, screen: ScreenId) -> Option<&ScreenPlacement> {
        self.shown.placement(&self.layout, screen)
    }

    /// The surfaces the layout and the kept frames hold.
    pub fn retained_surfaces(&self) -> Vec<u32> {
        self.shown.retained_surfaces(&self.layout)
    }

    /// Sends the app `messages` it answers with a frame (input, an action, a change it shows):
    /// asks it for one if there are any (`Pacing::need_if`), and queues them as `ready` readies
    /// them for the wire (be-host translates their ids and holds what waits for a report), or
    /// drops them where it answers None (an app that failed). Answers what to send now
    /// (`queue`).
    pub fn send_for_frame(
        &mut self,
        messages: Vec<Outbound>,
        ready: impl FnOnce(Vec<Outbound>) -> Option<Vec<Outbound>>,
    ) -> Vec<Outbound> {
        self.pacing.need_if(!messages.is_empty());
        if messages.is_empty() {
            return Vec::new();
        }
        match ready(messages) {
            Some(messages) => self.queue(messages),
            None => Vec::new(),
        }
    }

    /// Queues `messages` for the app. Answers what to send now: everything queued, once its
    /// session runs (`deliverable`).
    pub fn queue(&mut self, messages: impl IntoIterator<Item = Outbound>) -> Vec<Outbound> {
        self.queued.extend(messages);
        self.deliverable()
    }

    /// What is queued, taken once the session runs: until then it waits.
    pub fn deliverable(&mut self) -> Vec<Outbound> {
        match self.session.running() {
            true => std::mem::take(&mut self.queued),
            false => Vec::new(),
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
    /// what they wait for, its layout and the views it closed or left out (`Shown`), its
    /// presents, its frames in flight, the screens and paintings it was asked for, and what its
    /// instances' docks declared. Its views' accessibility panes stay, empty, and offer no
    /// actions until the new run describes them. It is told the fonts, whether to describe and
    /// the UTC offset again once it runs (`tell`); the host sends its instances' content again
    /// itself.
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
        self.layout = ScreenLayout::default();
        self.painting = false;
        self.told = Told::new(preferences);
        docks.restarted(app);
        described.grafts(&mut Graft::restarted);
        described.forget_actions();
    }
}

#[cfg(test)]
mod tests;
