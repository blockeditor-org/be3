//! A pan-and-zoom editor's zoom (`capabilities.pan_and_zoom`): the frame registers
//! `editor.zoom-in`, `editor.zoom-out`, `editor.zoom-reset` and `editor.zoom-fit` for it, by one
//! step (`beui::unstyled::ZOOM_STEP`), and `ZoomBar` shows them. "Fit" fits what the editor
//! keeps fitted (`Editor::keep_fitted`), or what it says fitting means (`Editor::fit_with`).

use std::rc::Rc;

use beui::NodeId;
use beui::dock::styled::{DockBarGrip, DockBarSurface};
use beui::icons::{ICON_SCAN, ICON_ZOOM_IN, ICON_ZOOM_OUT};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::ZOOM_STEP;

use crate::Editor;

/// The frame's zoom actions for one editor.
#[derive(Clone)]
pub struct ZoomActions {
    pub zoom_in: Action,
    pub zoom_out: Action,
    pub reset: Action,
    pub fit: Action,
    /// Whether the editor has a fit (`keep_fitted`, `fit_with`), which `fit` runs.
    pub fits: Memo<bool>,
}

impl PartialEq for ZoomActions {
    fn eq(&self, other: &Self) -> bool {
        self.zoom_in.key() == other.zoom_in.key()
    }
}

/// What "fit" does for an editor.
#[derive(Clone)]
pub(crate) struct Fitter(Rc<dyn Fn()>);

impl PartialEq for Fitter {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Editor {
    /// The frame's zoom actions, while the manifest declares `pan_and_zoom`.
    pub fn zoom_actions(&self) -> ReadSignal<Option<ZoomActions>> {
        self.zoom_state().0.read_only()
    }

    /// What `editor.zoom-fit` does here, for an editor whose "fit" is its own (the canvas's
    /// fit-all). One that keeps an area fitted (`keep_fitted`) needs none.
    pub fn fit_with(&self, fit: impl Fn() + 'static) {
        self.zoom_state().1.set(Some(Fitter(Rc::new(fit))));
    }
}

/// Registers the zoom actions for `editor`, a pan-and-zoom one, while `live`.
pub(crate) fn register(editor: Editor, live: Memo<bool>) -> ZoomActions {
    let (actions, fit) = editor.zoom_state();
    let scale = editor.scale();
    let fits = create_memo(move || fit.with(Option::is_some));
    let zoom = ZoomActions {
        zoom_in: Action::new("editor.zoom-in", "Zoom in", move || editor.zoom(ZOOM_STEP))
            .glyph(ICON_ZOOM_IN)
            .enabled(live)
            .register(),
        zoom_out: Action::new("editor.zoom-out", "Zoom out", move || {
            editor.zoom(ZOOM_STEP.recip())
        })
        .glyph(ICON_ZOOM_OUT)
        .enabled(live)
        .register(),
        reset: Action::new("editor.zoom-reset", "Zoom to 100%", move || {
            editor.zoom(scale.get_untracked().max(f32::EPSILON).recip())
        })
        .enabled(live)
        .register(),
        fit: Action::new("editor.zoom-fit", "Fit", move || {
            if let Some(Fitter(fit)) = fit.get_untracked() {
                fit();
            }
        })
        .glyph(ICON_SCAN)
        .enabled(create_memo(move || live.get() && fits.get()))
        .register(),
        fits,
    };
    actions.set(Some(zoom.clone()));
    zoom
}

/// The frame's zoom actions as a bar: out, the zoom (which resets it to 100%), in, and fit
/// where the editor has one. Empty for an editor that is not pan-and-zoom.
#[component]
pub fn ZoomBar(editor: Editor) -> NodeId {
    let actions = editor.zoom_actions();
    let scale = editor.scale();
    let readout = create_memo(move || format!("{:.0}%", scale.get() * 100.0));
    view! {
        <Frame>
            <Show value={actions}>
                |zoom: ReadSignal<ZoomActions>| {
                    let zoom = zoom.get_untracked();
                    let reset = zoom.reset.clone();
                    view! {
                        <DockBarSurface @test_id={"editor.zoom_bar"}>
                            <Frame @test_id={"editor.zoom_bar.grip"}>
                                <DockBarGrip />
                            </Frame>
                            <Button
                                icon_only=true
                                action={zoom.zoom_out.clone()}
                                @test_id={"editor.zoom_out"}
                            />
                            <Button
                                label={readout}
                                variant=ButtonVariant::Ghost
                                on_click={move || {
                                    reset.run();
                                }}
                                @test_id={"editor.zoom"}
                            />
                            <Button
                                icon_only=true
                                action={zoom.zoom_in.clone()}
                                @test_id={"editor.zoom_in"}
                            />
                            <Show value={zoom.fits}>
                                <Button
                                    icon_only=true
                                    action={zoom.fit.clone()}
                                    @test_id={"editor.zoom_fit"}
                                />
                            </Show>
                        </DockBarSurface>
                    }
                }
            </Show>
        </Frame>
    }
}
