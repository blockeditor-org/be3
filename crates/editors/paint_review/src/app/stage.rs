use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, ForEach, Frame, ItemSize, List, Memo, Picture, Show, clone, component,
    create_effect, create_memo, create_timer, set_component_state, untrack, view,
};
use block_editor_plugin::beui::styled::{Body, Caption, use_theme};
use block_editor_plugin::beui::{Document, ImageFit, NodeId, Pos2, Rect, Vec2};

use crate::render::Rendered;

use super::state::{Review, Shown};

#[derive(Clone)]
struct StageState {
    rastered: Memo<usize>,
    frame: Memo<usize>,
    scale: Memo<f32>,
    busy: Memo<bool>,
}

#[component]
pub(crate) fn Stage(review: Rc<Review>, count: Memo<usize>) -> NodeId {
    let shown = Rc::clone(&review);
    let frame = review.frame.clone();
    let revision = review.revision.clone();
    let painting = create_memo(clone!(shown frame revision -> move || {
        let _ = revision.get();
        let Some(path) = shown.selected.get() else {
            return Shown::Waiting(Some("Choose a painting to review it".to_owned()));
        };
        let Some(status) = shown.status(&path) else {
            return Shown::Waiting(None);
        };
        if shown.pending.get().is_some() {
            return Shown::Waiting(Some(
                "Waiting for the painting you approved before to arrive".to_owned(),
            ));
        }
        shown.shown(&path, shown.shown_as(status), frame.get())
    }));

    let clamped = Rc::clone(&review);
    let clamping = count.clone();
    create_effect(move || {
        let count = clamping.get();
        clamped.frame.with(|_| ());
        untrack(|| clamped.clamp_frame(count));
    });

    let advanced = Rc::clone(&review);
    let advancing = painting.clone();
    let advancing_count = count.clone();
    let stepper = create_timer(move || {
        let ready = !matches!(advancing.get_untracked(), Shown::Waiting(_));
        advanced.advance(advancing_count.get_untracked(), ready)
    });
    let scheduled = Rc::clone(&review);
    let scheduling = painting.clone();
    let scheduling_count = count.clone();
    create_effect(move || {
        scheduled.playing.with(|_| ());
        scheduled.frame.with(|_| ());
        let ready = !matches!(scheduling.get(), Shown::Waiting(_));
        let count = scheduling_count.get();
        untrack(|| match scheduled.next_advance(count, ready) {
            Some(delay) => stepper.start(delay),
            None => stepper.stop(),
        });
    });

    let panels = create_memo(clone!(painting -> move || match painting.get() {
        Shown::Ready(panels, _) => panels,
        _ => Vec::new(),
    }));
    let description = create_memo(clone!(painting -> move || match painting.get() {
        Shown::Ready(_, description) => Some(description),
        _ => None,
    }));
    let message = create_memo(clone!(painting -> move || match painting.get() {
        Shown::Failed(error) => Some((error, true)),
        Shown::Waiting(error) => Some((
            error.unwrap_or_else(|| "Rastering the painting".to_owned()),
            false,
        )),
        Shown::Ready(..) => None,
    }));
    let stalled = create_memo(clone!(message -> move || message.get().is_some()));
    let note = create_memo(clone!(message -> move || {
        message.get().map(|(text, _)| text).unwrap_or_default()
    }));
    let broken = create_memo(clone!(message -> move || {
        message.get().is_some_and(|(_, failed)| failed)
    }));

    let counted = Rc::clone(&review);
    let at = review.frame.clone();
    let zoom = review.scale.clone();
    let working = Rc::clone(&review);
    set_component_state(StageState {
        rastered: create_memo(clone!(revision -> move || {
            let _ = revision.get();
            counted.rastered()
        })),
        frame: create_memo(clone!(at -> move || at.get())),
        scale: create_memo(clone!(zoom -> move || zoom.get().unwrap_or_default())),
        busy: create_memo(clone!(revision working -> move || {
            let _ = revision.get();
            working.downloading.get()
                || working.pending.get().is_some()
                || working.loading().is_some()
        })),
    });

    let view = review.editor().canvas();
    let placed = review.editor().clone();
    let laid_out = panels.clone();
    let world = review.editor().world();
    let laid = create_memo(clone!(laid_out world -> move || {
        let panels = laid_out.get();
        if panels.is_empty() {
            return None;
        }
        let available = world
            .get()
            .unwrap_or_else(|| placed.content_rect().size())
            .max(Vec2::new(1.0, 1.0));
        let region = Rect::from_min_size(Pos2::ZERO, available);
        let laid = crate::view::laid_out(&panels, region);
        Some((
            laid.scale,
            laid.panels
                .into_iter()
                .map(|panel| panel.rect)
                .collect::<Vec<Rect>>(),
        ))
    }));
    let layout = create_memo(clone!(laid -> move || {
        laid.get().map(|(_, panels)| panels).unwrap_or_default()
    }));
    let reported = Rc::clone(&review);
    let camera = review.editor().scale();
    create_effect(clone!(laid description camera -> move || {
        let zoom = camera.get();
        reported.report(
            laid.get().map(|(scale, _)| scale * zoom),
            description.get(),
        );
    }));

    let places =
        create_memo(clone!(layout -> move || (0..layout.get().len()).collect::<Vec<usize>>()));
    let theme = use_theme();
    let loading = Rc::clone(&review);
    let progress = create_memo(clone!(loading revision -> move || {
        let _ = revision.get();
        match loading.loading() {
            Some((done, total)) => format!("Rastering frame {} of {total}", done + 1),
            None => String::new(),
        }
    }));

    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Canvas view={view} @sizing=ItemSize::Percent(100.0)>
                    <ForEach keys={places}>
                        {move |index: usize| {
                            let panels = panels.clone();
                            let layout = layout.clone();
                            let image = create_memo(move || {
                                panels.get().get(index).map(|panel: &Rendered| {
                                    panel.image.clone()
                                })
                            });
                            let rect = create_memo(move || {
                                layout.get().get(index).copied().unwrap_or(Rect::ZERO)
                            });
                            let x = create_memo(clone!(rect -> move || rect.get().min.x));
                            let y = create_memo(clone!(rect -> move || rect.get().min.y));
                            let width = create_memo(clone!(rect -> move || rect.get().width()));
                            let height = create_memo(clone!(rect -> move || rect.get().height()));
                            view! {
                                <CanvasItem x={x} y={y} width={width} height={height}>
                                    <Picture image={image} fit=ImageFit::Fill smooth=false />
                                </CanvasItem>
                            }
                        }}
                    </ForEach>
                </Canvas>
                <Notice shown={stalled} note={note} failed={broken} progress={progress} />
            </List>
        </Frame>
    }
}

#[component]
fn Notice(
    shown: Memo<bool>,
    note: Memo<String>,
    failed: Memo<bool>,
    progress: Memo<String>,
) -> NodeId {
    let theme = use_theme();
    let color = create_memo(clone!(theme failed -> move || match failed.get() {
        true => theme.danger.get(),
        false => theme.text_muted.get(),
    }));
    let telling = create_memo(clone!(progress -> move || !progress.get().is_empty()));
    view! {
        <List spacing=4.0>
            <Show condition={shown}>
                <Body content={note} color={color} @test_id={"paint_review.notice"} />
            </Show>
            <Show condition={telling}>
                <Caption content={progress} color={theme.text_muted.clone()} />
            </Show>
        </List>
    }
}

pub fn rastered(document: &Document, node: NodeId) -> usize {
    document.component_state::<StageState>(node).rastered.get()
}

pub fn frame_shown(document: &Document, node: NodeId) -> usize {
    document.component_state::<StageState>(node).frame.get()
}

pub fn zoom(document: &Document, node: NodeId) -> f32 {
    document.component_state::<StageState>(node).scale.get()
}

pub fn busy(document: &Document, node: NodeId) -> bool {
    document.component_state::<StageState>(node).busy.get()
}
