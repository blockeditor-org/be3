//! The layout and present holds (guides/apps/runtime.md, Layout and present commits): what of an
//! app's layout commit waits for the present that follows it (`Holds`, `Laid`), the last frames
//! of views its layout leaves out while the host still shows them (`Shown`'s kept frames), the
//! anchored views it closed until the frame that closed them is drawn (`Shown`'s closing), and
//! the damage of the presents a host shows (`Presents`).

use std::collections::VecDeque;

use be_lib_stable_hash::{HashMap, HashSet};
use be_sdk_api::{
    ChildPlacements, DockMessage, EditorInstanceId, LayoutCommit, PresentedFrame, ScreenDamage,
    ScreenId, ScreenLayout, ScreenPlacement, ScreenWindow, ViewId, ViewSize,
};

const REMEMBERED_PRESENTS: usize = 16;
/// How many hidden views of one app keep their last frame at once: past it, the one hidden
/// longest lets its frame go, and if it is shown again it may show empty for a frame while its
/// app draws it.
pub const MOST_KEPT: usize = 8;

/// What of a layout commit waits for the poll after the present that follows it.
pub struct Laid {
    pub serial: u64,
    /// How many presents the app must have sent before this is applied: the one that follows
    /// the layout commit it came in. Children come and go only with the frame that punches or
    /// fills their space, so a closed child never leaves its hole in the placer's old frame.
    pub presented_by: u64,
    /// The sequence of the last frame drawn before its layout: a frame past it may paint what
    /// these children go with, so it waits for them (`Holds::waits_for_children`), even when it
    /// arrives before the present that brings it is heard.
    pub drawn_after: u64,
    pub sizes: Vec<ViewSize>,
    pub windows: Vec<ScreenWindow>,
    pub children: Vec<ChildPlacements>,
    /// The commit's dock messages, applied after its children: which tabs a root shows decides
    /// which of the children it places are drawn, so a tab switched to and the one it hides
    /// change in the same frame.
    pub docks: Vec<DockMessage>,
}

impl Laid {
    /// What a layout commit applied at once lays out.
    pub fn now(
        serial: u64,
        sizes: Vec<ViewSize>,
        windows: Vec<ScreenWindow>,
        children: Vec<ChildPlacements>,
    ) -> Self {
        Self {
            serial,
            presented_by: 0,
            drawn_after: 0,
            sizes,
            windows,
            children,
            docks: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sizes.is_empty()
            && self.windows.is_empty()
            && self.children.is_empty()
            && self.docks.is_empty()
    }

    /// The screens it lays out, with the screen of each placer's view from `screen_of`.
    pub fn screens(
        &self,
        screen_of: impl Fn(EditorInstanceId, ViewId) -> Option<ScreenId>,
    ) -> HashSet<ScreenId> {
        self.sizes
            .iter()
            .map(|size| size.screen)
            .chain(self.windows.iter().map(|window| window.screen))
            .chain(
                self.children
                    .iter()
                    .filter_map(|placements| screen_of(placements.instance, placements.view)),
            )
            .collect()
    }
}

/// An answer held for a later poll: what a layout laid out, or another answer `A` heard after it.
pub enum Held<A> {
    Laid(Laid),
    Answer(A),
}

/// The answers an app sent that wait for a later poll, in order, and the counts of what the
/// host has heard that release them.
pub struct Holds<A> {
    held: Vec<Held<A>>,
    /// The sequence of the last present heard that painted: frames up to it were drawn before
    /// any layout heard since (`Laid::drawn_after`).
    painted_heard: u64,
    layouts_heard: u64,
    presents_heard: u64,
}

impl<A> Default for Holds<A> {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            painted_heard: 0,
            layouts_heard: 0,
            presents_heard: 0,
        }
    }
}

impl<A> Holds<A> {
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    pub fn len(&self) -> usize {
        self.held.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Held<A>> {
        self.held.iter()
    }

    /// What every held layout lays out.
    pub fn laid(&self) -> impl Iterator<Item = &Laid> {
        self.held.iter().filter_map(|held| match held {
            Held::Laid(laid) => Some(laid),
            Held::Answer(_) => None,
        })
    }

    /// Forgets everything held and heard, as when the app restarts.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The app sent `presents` presents, the last that painted being `painted`.
    pub fn heard_presents(&mut self, presents: u64) {
        self.presents_heard += presents;
    }

    pub fn heard_painted(&mut self, sequence: u64) {
        self.painted_heard = self.painted_heard.max(sequence);
    }

    /// Takes a layout commit apart: what draws the frame stays in `commit`, to apply at once,
    /// and the rest is held; true when something was. Children that only move
    /// (`only_moves`) are applied at once, with the painting that goes with them; when any
    /// child comes or goes the painting waits for the poll, so the ones that move, and the
    /// views' sizes (a fit child's), wait with it.
    pub fn heard_layout(
        &mut self,
        commit: &mut LayoutCommit,
        only_moves: impl Fn(&ChildPlacements) -> bool,
    ) -> bool {
        self.layouts_heard += 1;
        let moves = |placements: &ChildPlacements| {
            !self.laid().any(|laid| {
                laid.children.iter().any(|earlier| {
                    (earlier.instance, earlier.view) == (placements.instance, placements.view)
                })
            }) && only_moves(placements)
        };
        let (coming, sizes) = match commit.children.iter().all(moves) {
            true => (Vec::new(), Vec::new()),
            false => (
                std::mem::take(&mut commit.children),
                std::mem::take(&mut commit.sizes),
            ),
        };
        let mut later = Laid {
            serial: commit.serial,
            presented_by: self.layouts_heard,
            drawn_after: self.painted_heard,
            sizes,
            windows: std::mem::take(&mut commit.windows),
            children: coming,
            docks: Vec::new(),
        };
        if self.laid().next().is_some() || !later.is_empty() {
            later.docks = std::mem::take(&mut commit.docks);
        }
        if later.is_empty() {
            return false;
        }
        self.held.push(Held::Laid(later));
        true
    }

    /// Holds an answer heard after a held layout, so it is applied after it.
    pub fn hold(&mut self, answer: A) {
        self.held.push(Held::Answer(answer));
    }

    /// Takes the held answers in order, up to the first layout still waiting on the frame it
    /// came with.
    pub fn release_presented(&mut self) -> Vec<Held<A>> {
        let presents_heard = self.presents_heard;
        let waiting = self.held.iter().position(
            |held| matches!(held, Held::Laid(laid) if laid.presented_by > presents_heard),
        );
        let rest = self.held.split_off(waiting.unwrap_or(self.held.len()));
        std::mem::replace(&mut self.held, rest)
    }

    /// Whether the frame of present `sequence` comes after the layout of children still held.
    pub fn waits_for_children(&self, sequence: u64) -> bool {
        self.laid().any(|laid| sequence > laid.drawn_after)
    }
}

/// The app's screens while anchored views they leave out are still shown: until the present
/// after the layout (`presented_by`, counted in presents applied) and, when it painted, the frame
/// of that present's sequence (`drawn_by`).
struct Closing {
    presented_by: u64,
    drawn_by: Option<u64>,
    layout: ScreenLayout,
}

/// The last frame of a view the app's layout left out because the host stopped showing it (a
/// dock tab or bar hidden with its tab), while the host still holds the view: shown again, the
/// view draws this until the app's new frame of it has been drawn, so it never shows empty while
/// that frame is on its way.
struct Kept {
    placement: ScreenPlacement,
    /// When it was hidden, counted in hides, so the oldest goes first past `MOST_KEPT`.
    hidden: u64,
    /// How far the app has come drawing the view again, once it lays its screen out again.
    back: Option<Back>,
}

/// A kept frame goes once the present that follows the layout bringing its view back has been
/// applied and, where it painted, its frame has been drawn, as an anchored view that closes does
/// (`Closing`).
#[derive(Clone, Copy)]
enum Back {
    /// Laid out again; waits for this many presents applied.
    Laid(u64),
    /// That present came, with the sequence of its frame if it painted.
    Presented(Option<u64>),
}

/// What a present applied or a frame shown lets go (`Shown::presented`, `Shown::frame_shown`).
#[must_use]
pub struct Release {
    /// The anchored views a layout closed may go: lay out these screens.
    pub closed: Option<ScreenLayout>,
    /// Kept frames may go: let go of those drawn anew (`Shown::release_kept`).
    pub kept: bool,
}

/// What of an app's screens a host shows beyond its last layout: anchored views it closed, until
/// the frame that closed them is drawn, and the last frames of views it left out while the host
/// still holds them.
pub struct Shown {
    /// Whether the host holds the textures of what the app presented (natively), so it can keep
    /// a frame and wait for a present's pixels; in the browser a surface is the app worker's
    /// canvas, which goes when the app's layout leaves it out.
    holds_textures: bool,
    /// Presents applied, in the order the app sent them among its other answers.
    presents_applied: u64,
    closing: Option<Closing>,
    kept: HashMap<ScreenId, Kept>,
    /// Counts the views hidden, to order the kept frames (`Kept::hidden`).
    hides: u64,
}

impl Shown {
    pub fn new(holds_textures: bool) -> Self {
        Self {
            holds_textures,
            presents_applied: 0,
            closing: None,
            kept: HashMap::default(),
            hides: 0,
        }
    }

    /// Forgets what the app showed, as when it restarts.
    pub fn clear(&mut self) {
        self.presents_applied = 0;
        self.closing = None;
        self.kept.clear();
    }

    /// Takes the app's new screens, replacing `old`: returns what to lay out now. An anchored
    /// view (`anchored`) they leave out stays, showing its last frame, until the frame that
    /// closed it is drawn (`closed`): what the view showed may have moved into another of the
    /// app's surfaces (a dragged dock bar settling back into its band), which shows only with
    /// that frame, so it is never missing in between.
    pub fn place(
        &mut self,
        old: &ScreenLayout,
        layout: ScreenLayout,
        anchored: impl Fn(ScreenId) -> bool,
    ) -> ScreenLayout {
        self.closing = None;
        let closed: Vec<_> = old
            .screens
            .iter()
            .filter(|placement| {
                layout.placement(placement.screen).is_none() && anchored(placement.screen)
            })
            .cloned()
            .collect();
        if closed.is_empty() {
            return layout;
        }
        let mut shown = layout.clone();
        shown.screens.extend(closed);
        self.closing = Some(Closing {
            presented_by: self.presents_applied + 1,
            drawn_by: None,
            layout,
        });
        shown
    }

    /// The app's present was applied, with the sequence of its frame if it painted, while the
    /// last present shown is `shown`: what may now be let go.
    pub fn presented(&mut self, painted: Option<u64>, shown: u64) -> Release {
        self.presents_applied += 1;
        let holds_textures = self.holds_textures;
        if let Some(closing) = &mut self.closing
            && self.presents_applied == closing.presented_by
        {
            closing.drawn_by = painted.filter(|_| holds_textures);
        }
        for kept in self.kept.values_mut() {
            if let Some(Back::Laid(presented_by)) = kept.back
                && self.presents_applied >= presented_by
            {
                kept.back = Some(Back::Presented(painted));
            }
        }
        self.release(shown)
    }

    /// The host showed the frame of present `shown`: what may now be let go.
    pub fn frame_shown(&mut self, shown: u64) -> Release {
        self.release(shown)
    }

    fn release(&mut self, shown: u64) -> Release {
        Release {
            closed: self.closed(shown),
            kept: !self.kept.is_empty(),
        }
    }

    /// The screens to lay out once the frame that closed anchored views has been presented and,
    /// where the host holds its textures, drawn (`shown`, the last present shown).
    fn closed(&mut self, shown: u64) -> Option<ScreenLayout> {
        let closing = self.closing.as_ref()?;
        let presented = self.presents_applied >= closing.presented_by;
        let drawn = closing.drawn_by.is_none_or(|sequence| shown >= sequence);
        if !presented || !drawn {
            return None;
        }
        self.closing.take().map(|closing| closing.layout)
    }

    /// Keeps the last frame of each view of `old` that `layout` leaves out and the host still
    /// holds (`holds`), and notes when the app lays out again a view whose last frame is kept.
    pub fn keep_left_out(
        &mut self,
        old: &ScreenLayout,
        layout: &ScreenLayout,
        holds: impl Fn(&ScreenPlacement) -> bool,
    ) {
        if !self.holds_textures {
            return;
        }
        for placement in &old.screens {
            if !holds(placement) || layout.placement(placement.screen).is_some() {
                continue;
            }
            self.hides += 1;
            let hidden = self.hides;
            self.kept
                .entry(placement.screen)
                .and_modify(|kept| {
                    kept.back = None;
                    kept.hidden = hidden;
                })
                .or_insert(Kept {
                    placement: *placement,
                    hidden,
                    back: None,
                });
        }
        // Only the views hidden last keep their frame; the textures of the others go with the
        // next `release_kept`. One this layout shows again is coming back, so it keeps its frame.
        let mut hidden: Vec<(u64, ScreenId)> = self
            .kept
            .iter()
            .filter(|(screen, kept)| kept.back.is_none() && layout.placement(**screen).is_none())
            .map(|(screen, kept)| (kept.hidden, *screen))
            .collect();
        if hidden.len() > MOST_KEPT {
            hidden.sort_unstable();
            for (_, screen) in &hidden[..hidden.len() - MOST_KEPT] {
                self.kept.remove(screen);
            }
        }
        let next = self.presents_applied + 1;
        for placement in &layout.screens {
            if let Some(kept) = self.kept.get_mut(&placement.screen)
                && kept.back.is_none()
            {
                kept.back = Some(Back::Laid(next));
            }
        }
    }

    /// Lets go of each kept frame whose view the host no longer holds (`holds`) or whose new
    /// frame has been drawn (`shown`, the last present shown).
    pub fn release_kept(&mut self, shown: u64, holds: impl Fn(ScreenId) -> bool) {
        self.kept.retain(|screen, kept| {
            let drawn = match kept.back {
                None | Some(Back::Laid(_)) => false,
                // A present that painted nothing lets the kept frame go though the new surface has
                // no pixels yet, as a closing anchored view does (`Closing`).
                Some(Back::Presented(painted)) => painted.is_none_or(|sequence| shown >= sequence),
            };
            holds(*screen) && !drawn
        });
    }

    pub fn keeps(&self, screen: ScreenId) -> bool {
        self.kept.contains_key(&screen)
    }

    pub fn kept_placements(&self) -> Vec<ScreenPlacement> {
        self.kept.values().map(|kept| kept.placement).collect()
    }

    /// The surfaces `layout` and the kept frames hold.
    pub fn retained_surfaces(&self, layout: &ScreenLayout) -> Vec<u32> {
        layout
            .screens
            .iter()
            .chain(self.kept.values().map(|kept| &kept.placement))
            .map(|placement| placement.surface)
            .collect()
    }

    /// The placement a screen is drawn from: its kept last frame while it has one, else
    /// `layout`'s.
    pub fn placement<'a>(
        &'a self,
        layout: &'a ScreenLayout,
        screen: ScreenId,
    ) -> Option<&'a ScreenPlacement> {
        self.kept
            .get(&screen)
            .map(|kept| &kept.placement)
            .or_else(|| layout.placement(screen))
    }
}

/// The presents an app reported, until the host shows the frame they make.
#[derive(Default)]
pub struct Presents {
    reported: VecDeque<PresentedFrame>,
    shown: u64,
}

impl Presents {
    pub fn report(&mut self, presented: PresentedFrame) {
        if self.reported.len() == REMEMBERED_PRESENTS {
            self.reported.pop_front();
        }
        self.reported.push_back(presented);
    }

    /// The sequence of the last present shown.
    pub fn shown(&self) -> u64 {
        self.shown
    }

    /// The damage of the presents since the last shown through `presents`, where every one of
    /// them reported it, and the placements their surfaces were painted at, in order.
    pub fn damage_through(
        &mut self,
        presents: u64,
    ) -> (Option<Vec<ScreenDamage>>, Vec<ScreenPlacement>) {
        let since = std::mem::replace(&mut self.shown, presents);
        let mut damage = Vec::new();
        let mut painted = Vec::new();
        let mut found = 0;
        for presented in &self.reported {
            if presented.sequence <= presents {
                painted.extend_from_slice(&presented.painted);
            }
            if presented.sequence > since && presented.sequence <= presents {
                damage.extend_from_slice(&presented.damage);
                found += 1;
            }
        }
        self.reported
            .retain(|presented| presented.sequence > presents);
        (
            (presents > since && found == presents - since).then_some(damage),
            painted,
        )
    }
}

#[cfg(test)]
mod tests;
