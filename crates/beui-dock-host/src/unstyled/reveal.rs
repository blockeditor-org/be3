//! Bands of the dock's own bars that hide until the dock's owner reveals them: bands the user
//! auto-hid (`DockState::is_edge_auto_hidden`) and, where the owner offers hidden bands
//! (`DockHostLayout::offer_hidden_bands`), every band while a full screen tab or something outside
//! covers the dock. A hidden band takes no room from the body. The owner hears where each one is
//! (`DockHostLayout::hidden_bands`) and says how far it is out (`DockHostLayout::reveal`); it slides
//! in from its edge in its own layer at `LayerLevel::Raised`, over everything the dock shows. A bar
//! keeps its band out while it holds it (`DockBarControl::hold_band`), as a bar's open panel does.

use be_lib_stable_hash::HashMap;

use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::{Align2, Direction, Extent, LayerLevel, LayerPolicy, Placement};
use beui_view::types::{Rect, Vec2, pos2, vec2};

use super::bars::{BarHolder, DockBarBandHandle, DockBarRun};
use super::{DockAlign, DockBarPlacement, DockEdge, State, TabId};

/// How thick a band is taken to be before it was ever shown and measured.
const UNMEASURED: f32 = 48.0;

/// A hidden band of the dock's own bars along `edge`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HiddenBand {
    pub edge: DockEdge,
    /// Where the band is once fully out, in the dock's window.
    pub rect: Rect,
    /// It hides because a full screen tab or something outside covers the dock, not because the
    /// user auto-hid it.
    pub covered: bool,
    /// A bar in it holds it out.
    pub held: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct BandReveal {
    pub(crate) offered: Signal<bool>,
    pub(crate) amounts: Signal<HashMap<DockEdge, f32>>,
    pub(crate) bands: Signal<Vec<HiddenBand>>,
    pub(crate) holds: Signal<Vec<TabId>>,
}

impl BandReveal {
    pub(crate) fn new() -> Self {
        Self {
            offered: create_signal(false),
            amounts: create_signal(HashMap::default()),
            bands: create_signal(Vec::new()),
            holds: create_signal(Vec::new()),
        }
    }

    pub(crate) fn amount(&self, edge: DockEdge) -> f32 {
        self.amounts
            .with(|amounts| amounts.get(&edge).copied().unwrap_or(0.0))
    }

    pub(crate) fn set_amount(&self, edge: DockEdge, amount: f32) {
        let amount = amount.clamp(0.0, 1.0);
        if untrack(|| self.amount(edge)) != amount {
            self.amounts.update(|amounts| {
                amounts.insert(edge, amount);
            });
        }
    }

    pub(crate) fn hold(&self, bar: TabId, held: bool) {
        if !self.holds.is_alive() || self.holds.with_untracked(|holds| holds.contains(&bar)) == held
        {
            return;
        }
        self.holds.update(|holds| match held {
            true => holds.push(bar),
            false => holds.retain(|held| *held != bar),
        });
    }

    fn report(&self, edge: DockEdge, band: Option<HiddenBand>) {
        if !self.bands.is_alive() {
            return;
        }
        let held = self
            .bands
            .with_untracked(|bands| bands.iter().find(|held| held.edge == edge).copied());
        if held == band {
            return;
        }
        self.bands.update(|bands| {
            bands.retain(|held| held.edge != edge);
            bands.extend(band);
        });
    }
}

impl State {
    /// A full screen tab or something outside covers the dock and its bars.
    pub(super) fn bars_covered(&self) -> bool {
        self.fullscreen_over_bars() || self.covered.get()
    }

    /// Whether the band of the dock's own bars along `edge` hides until it is revealed.
    pub(super) fn conceals(&self, edge: DockEdge) -> bool {
        self.state.with(|state| state.is_edge_auto_hidden(edge))
            || (self.reveal.offered.get() && self.bars_covered())
    }

    pub(super) fn offers_auto_hide(&self, bar: TabId) -> bool {
        self.reveal.offered.get()
            && self.state.with(|state| {
                state.is_dock_bar(bar) && state.bar_placement(bar).is_some_and(|at| at.is_beside())
            })
    }
}

#[component]
pub(super) fn DockBarHidden(dock: State, edge: DockEdge) -> NodeId {
    let bars = dock.bars_at(BarHolder::Dock, edge, false, move |placement| {
        placement.is_beside() && placement.edge() == edge
    });
    let reveal = dock.reveal;
    let concealed = create_memo(move || !bars.with(Vec::is_empty) && dock.conceals(edge));
    let held = create_memo(move || {
        reveal
            .holds
            .with(|holds| bars.with(|bars| bars.iter().any(|bar| holds.contains(bar))))
    });
    let amount = create_memo(move || match held.get() {
        true => 1.0,
        false => reveal.amount(edge),
    });
    let thickness = create_signal(None::<f32>);
    let area = dock.rect;
    let full = create_memo(move || {
        let rect = area.get();
        let thick = thickness.get().unwrap_or(UNMEASURED);
        match edge {
            DockEdge::Top => Rect::from_min_size(rect.min, vec2(rect.width(), thick)),
            DockEdge::Bottom => Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - thick),
                vec2(rect.width(), thick),
            ),
            DockEdge::Left => Rect::from_min_size(rect.min, vec2(thick, rect.height())),
            DockEdge::Right => Rect::from_min_size(
                pos2(rect.right() - thick, rect.top()),
                vec2(thick, rect.height()),
            ),
        }
    });
    create_effect(move || {
        let band = concealed.get().then(|| HiddenBand {
            edge,
            rect: full.get(),
            covered: dock.bars_covered(),
            held: held.get(),
        });
        untrack(|| reveal.report(edge, band));
    });
    on_cleanup(move || reveal.report(edge, None));
    let across = move |size: Vec2| match edge.direction() {
        Direction::Horizontal => size.y,
        Direction::Vertical => size.x,
    };
    let translate = create_memo(move || {
        let away = (1.0 - amount.get()) * across(full.get().size());
        match edge {
            DockEdge::Top => vec2(0.0, -away),
            DockEdge::Bottom => vec2(0.0, away),
            DockEdge::Left => vec2(-away, 0.0),
            DockEdge::Right => vec2(away, 0.0),
        }
    });
    let width = create_memo(move || match edge.direction() {
        Direction::Horizontal => Extent::px(full.get().width()),
        Direction::Vertical => Extent::INTRINSIC,
    });
    let height = create_memo(move || match edge.direction() {
        Direction::Horizontal => Extent::INTRINSIC,
        Direction::Vertical => Extent::px(full.get().height()),
    });
    let open = create_memo(move || concealed.get() && amount.get() > 0.0);
    let anchor = create_memo(move || OverlayAnchor::Point(full.get().min));
    let layer = NodeRef::new();
    let view = view! {
        <Overlay
            @node_ref=&layer
            anchor={anchor}
            placement={Placement::Over(0)}
            policy=LayerPolicy::Pass
            level=LayerLevel::Raised
            traps_focus=false
            open={open}
        >|| {
            let ends = create_signal([0.0_f32; 2]);
            let body = view! {
                <Layers>
                    <DockBarRun
                        dock
                        holder={BarHolder::Dock}
                        edge
                        placement={DockBarPlacement::Beside(edge, DockAlign::Start)}
                        ends
                    />
                    <DockBarRun
                        dock
                        holder={BarHolder::Dock}
                        edge
                        placement={DockBarPlacement::Beside(edge, DockAlign::Center)}
                        ends
                    />
                    <DockBarRun
                        dock
                        holder={BarHolder::Dock}
                        edge
                        placement={DockBarPlacement::Beside(edge, DockAlign::End)}
                        ends
                    />
                </Layers>
            };
            let band = dock.bar_band.call(DockBarBandHandle { edge, body });
            let measured = measure(band);
            create_effect(move || {
                let Some(thick) = measured.get().map(|measured| across(measured.rect.size())) else {
                    return;
                };
                if thick > 0.0 && untrack(|| thickness.get()) != Some(thick) {
                    thickness.set(Some(thick));
                }
            });
            view! {
                <Frame align=Align2::START>
                    <Frame width={width} height={height} translate={translate}>{band}</Frame>
                </Frame>
            }
        }</Overlay>
    };
    create_effect(move || {
        if open.get() {
            untrack(|| dock.raise_window(layer));
        }
    });
    view
}
