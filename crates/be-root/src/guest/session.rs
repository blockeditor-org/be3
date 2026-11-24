//! The session's view (plans/host_as_platform_adapter.md, 4g7b2): the editor of the block the
//! session shows (core-view's `showing`), which the root's host opens and serves
//! (`Embedded::spawn_block`), filling the window under the root's own UI. While its app loads,
//! or once it stopped or could not start, a notice shows over it in be-host's own words
//! (its `app_view`'s `Notices`); Reload starts an app that stopped again.

use std::{cell::Cell, future::Future, pin::Pin, rc::Rc, task::Poll, time::Duration};

use be_lib_geometry::pos2;
use be_lib_layers::{Area, Layer, LayerInput};
use be_sdk_api::{
    EditorInstanceId, ParentView, Pos2 as ViewPos2, ScreenId, ScreenRequest, SurfaceFormat,
    SurfaceSpec, Vec2 as ViewVec2, ViewId, ViewPlacement, ViewportMetrics,
};
use be_sdk_root::{BlockSpawn, Embedded, Hosted, SpawnFailure};
use beui::icons::{ICON_CIRCLE_ALERT, ICON_REFRESH_CW};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, NodeId, TextAlign};

use super::{Root, Window, host};
use crate::{LOADING_APP, NOT_OPENED_TITLE, SESSION_NOTICE_LAYER};

/// The screen the root asks of the editor of the block its session shows: the window, under
/// everything else it shows.
pub(super) const SESSION_SCREEN: ScreenId = ScreenId(1);

/// The editor of the block the session shows, as far as it got.
pub(super) struct Session {
    pub(super) block: [u8; 16],
    /// Counts the sessions shown, so an app spawned for one that went is let go.
    pub(super) round: u64,
    pub(super) state: Showing,
}

pub(super) enum Showing {
    /// Its host has yet to name and open the editor, or its app to say hello.
    Loading,
    Running(Hosted),
    Failed(Failure),
}

/// Why the session's editor shows nothing, as be-host's own UI says it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Failure {
    pub(super) title: String,
    pub(super) detail: String,
    /// Whether Reload starts it again: an app that stopped, not one that could not start.
    pub(super) reload: bool,
}

/// What the session's notice shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum SessionShown {
    Loading,
    Failed(Failure),
}

impl Showing {
    fn notice(&self) -> Option<SessionShown> {
        match self {
            Self::Loading => Some(SessionShown::Loading),
            Self::Running(hosted) if !hosted.app.started() => Some(SessionShown::Loading),
            Self::Running(_) => None,
            Self::Failed(failure) => Some(SessionShown::Failed(failure.clone())),
        }
    }
}

impl Root {
    /// Shows the editor of the block the session shows, once the organization's blocks are
    /// open: started when that block changes, or again when Reload asked, and let go once the
    /// block goes. An app that stopped shows why. Its screen follows the window.
    pub(super) fn settle_session(&self, time: Duration) {
        let showing = {
            let core = self.core.borrow();
            core.waits_for.is_none().then_some(core.showing).flatten()
        };
        let reload = self.session_reload.take();
        let mut session = self.session.borrow_mut();
        let current = session.as_ref().map(|session| session.block);
        if current != showing || reload {
            if let Some(Session {
                state: Showing::Running(hosted),
                ..
            }) = session.take()
            {
                self.embedding.borrow_mut().remove(hosted.app.instance());
            }
            if let Some(block) = showing {
                let round = self.session_rounds.get() + 1;
                self.session_rounds.set(round);
                *session = Some(Session {
                    block,
                    round,
                    state: Showing::Loading,
                });
                be_wasm_ops::guest::spawn(self.clone().load_session(block, round, time));
            }
        }
        if let Some(Session {
            state: state @ Showing::Running(_),
            ..
        }) = &mut *session
            && let Showing::Running(hosted) = &*state
        {
            match hosted.app.failure() {
                Some(why) => {
                    let failure = Failure {
                        title: match hosted.app.started() {
                            true => format!("{} stopped", hosted.name),
                            false => format!("{} could not start", hosted.name),
                        },
                        detail: why,
                        reload: hosted.app.started(),
                    };
                    self.embedding.borrow_mut().remove(hosted.app.instance());
                    *state = Showing::Failed(failure);
                }
                None => hosted
                    .app
                    .show(vec![screen(hosted.editor, self.window.get())]),
            }
        }
        let notice = session
            .as_ref()
            .and_then(|session| session.state.notice());
        drop(session);
        if let (Some(surface), Some(shown)) = (
            &mut *self.session_notice.borrow_mut(),
            self.session_notice_shown.get(),
        ) && shown.get_untracked() != notice
        {
            surface.enter(|| shown.set(notice));
        }
    }

    /// Spawns the app that edits `block` and has the host open its editor in it, for the
    /// session shown as `round` at the frame clock's `time`.
    async fn load_session(self, block: [u8; 16], round: u64, time: Duration) {
        let window = self.window.clone();
        let spawned = Embedded::spawn_block(
            BlockSpawn {
                block,
                screens: Box::new(move |editor| vec![screen(editor, window.get())]),
                surface: SurfaceSpec {
                    format: SurfaceFormat::Rgba8Unorm,
                    render: true,
                },
                fonts: self.fonts.borrow().clone().unwrap_or_default(),
            },
            time,
        )
        .await;
        let mut held = self.session.borrow_mut();
        let Some(session) = held
            .as_mut()
            .filter(|session| session.round == round)
        else {
            // A session that went: the app goes with this, which stops it.
            return;
        };
        session.state = match spawned {
            Ok(hosted) => {
                self.embedding.borrow_mut().embed(hosted.app.clone());
                Showing::Running(hosted)
            }
            Err(SpawnFailure { name, why }) => Showing::Failed(Failure {
                title: match name {
                    Some(name) => format!("{name} could not start"),
                    None => NOT_OPENED_TITLE.to_owned(),
                },
                detail: why,
                reload: false,
            }),
        };
        drop(held);
        self.waker.wake();
    }

    /// The session's editor's app, while it runs: asked for a frame for `time`, which waits
    /// for it until the frame's deadline at the latest.
    pub(super) async fn session_frame(&self, time: Duration) -> Option<Layer> {
        let app = match &*self.session.borrow() {
            Some(Session {
                state: Showing::Running(hosted),
                ..
            }) => hosted.app.clone(),
            _ => return None,
        };
        let deadline = std::pin::pin!(host::deadline());
        let drawn = std::pin::pin!(app.frame(time));
        first(drawn, deadline).await;
        app.layer(SESSION_SCREEN, pos2(0.0, 0.0))
    }

    /// Whether the session's editor is still being opened: its host has yet to name and open
    /// it, so the root owes its host the frame that shows it (a host waiting for its root to
    /// settle waits for it). Once its app runs, the app's own pacing says what it owes.
    pub(super) fn session_loading(&self) -> bool {
        matches!(
            &*self.session.borrow(),
            Some(Session {
                state: Showing::Loading,
                ..
            })
        )
    }

    /// The session's editor's app while it runs, with the id of the layer showing its screen
    /// once it painted it.
    pub(super) fn session_screen(&self) -> Option<(Embedded, be_lib_layers::LayerId)> {
        let session = self.session.borrow();
        let Some(Session {
            state: Showing::Running(hosted),
            ..
        }) = &*session
        else {
            return None;
        };
        let layer = hosted.app.layer(SESSION_SCREEN, pos2(0.0, 0.0))?;
        Some((hosted.app.clone(), layer.id))
    }

    /// The session's notice over its editor, while it shows one: taking every press, for its
    /// Reload.
    pub(super) fn session_notice_layer(&self) -> Option<Layer> {
        let shown = self.session_notice_shown.get()?;
        shown.get_untracked()?;
        let surface = self.session_notice.borrow();
        let mut layer = surface.as_ref()?.layer(SESSION_NOTICE_LAYER, pos2(0.0, 0.0));
        layer.input = LayerInput {
            hit: Area::Whole,
            ..LayerInput::default()
        };
        Some(layer)
    }
}

/// The screen the session's editor `editor` is asked for: the whole window, at its pixels per
/// point.
fn screen(editor: EditorInstanceId, window: Window) -> ScreenRequest {
    let (pixel_width, pixel_height) = window.whole();
    ScreenRequest {
        screen: SESSION_SCREEN,
        instance: editor,
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: ViewVec2 {
                x: window.width,
                y: window.height,
            },
            visible: ViewPos2::ZERO,
            pixel_width,
            pixel_height,
            scale_factor: window.pixels_per_point,
            raster: window.pixels_per_point,
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
        fit: false,
        origin: None,
    }
}

/// Whether `left` finished before `right`, polling both until one does.
async fn first<A: Future, B: Future>(mut left: Pin<&mut A>, mut right: Pin<&mut B>) -> bool {
    std::future::poll_fn(|context| {
        if left.as_mut().poll(context).is_ready() {
            return Poll::Ready(true);
        }
        match right.as_mut().poll(context).is_ready() {
            true => Poll::Ready(false),
            false => Poll::Pending,
        }
    })
    .await
}

/// The session's notice: its app loading, or why it shows nothing with Reload when it can
/// start again, as be-host's `app_view` shows them (its `Notices` and `CrashNotice`).
#[component]
pub(super) fn SessionNotice(notice: Signal<Option<SessionShown>>, reload: Rc<Cell<bool>>) -> NodeId {
    let loading = create_memo(move || matches!(notice.get(), Some(SessionShown::Loading)));
    let failure = create_memo(move || match notice.get() {
        Some(SessionShown::Failed(failure)) => Some(failure),
        _ => None,
    });
    let title = create_memo(move || failure.get().map(|failure| failure.title).unwrap_or_default());
    let detail =
        create_memo(move || failure.get().map(|failure| failure.detail).unwrap_or_default());
    view! {
            <Frame align=Align::Center>
                <Show
                    value={failure}
                    fallback={move || view! {
                        <Show value={loading}>
                            <Frame padding=Space::Lg>
                                <List spacing=Space::Md align=Align::Center>
                                    <Text align=TextAlign::Center color=TextColor::Muted>
                                        {LOADING_APP}
                                    </Text>
                                    <Progress width=INLINE_PROGRESS_WIDTH />
                                </List>
                            </Frame>
                        </Show>
                    }}
                >|failure: ReadSignal<Failure>| {
                    let reload = Rc::clone(&reload);
                    let reloadable = create_memo(move || failure.with(|failure| failure.reload));
                    view! {
                        <Frame @test_id="session.crashed" padding=Space::Xl>
                            <List spacing=Space::Md align=Align::Center>
                                <Text
                                    size=TextSize::Xl
                                    color=TextColor::Danger
                                    align=TextAlign::Center
                                >
                                    <Icon glyph=ICON_CIRCLE_ALERT />
                                </Text>
                                <Text style=TextStyle::HEADING align=TextAlign::Center>
                                    {title}
                                </Text>
                                <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                                    {detail}
                                </Text>
                                <Show value={reloadable}>|| {
                                    let reload = Rc::clone(&reload);
                                    view! {
                                        <Button
                                            @test_id="session.crashed.reload"
                                            label="Reload"
                                            glyph=ICON_REFRESH_CW
                                            variant=ButtonVariant::Secondary
                                            on_click={move || reload.set(true)}
                                        />
                                    }
                                }</Show>
                            </List>
                        </Frame>
                    }
                }</Show>
            </Frame>
    }
}
