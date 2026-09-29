use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::{
    Canvas, CanvasItem, CanvasView, ClickCatcher, Draw, Drawing, Focusable, ForEach, Memo, Prop,
    ReadSignal, Text, clone, component, create_effect, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{Theme, use_theme};
use block_editor_beui::beui::{
    Color32, Key, KeyPress, NodeId, Painter, PointerPress, Pos2, Rect, SecondaryDrag, Vec2,
};
use game_api::Spot;
use game_api::board::Sprite;

use super::Steps;
use super::annotations::{self, Annotation, Brush, toggled};
use super::layout::{CARD, LABEL_HEIGHT, Layout, Look, Placed};
use super::play::{Dragging, Mark, Play};
use super::skin::paint_sprite;

const DRAG_THRESHOLD: f32 = 6.0;
const LABEL_SIZE: f32 = 13.0;
const CARD_RADIUS: f32 = 6.0;
const CELL_RADIUS: f32 = 8.0;
const PLATE_RADIUS: f32 = 10.0;
const MARK_WIDTH: f32 = 3.0;
const BOARD_EDGE: Color32 = Color32::from_rgb(92, 64, 46);
const CHOSEN: Color32 = Color32::from_rgba_unmultiplied(20, 85, 30, 110);
const LANDING: Color32 = Color32::from_rgba_unmultiplied(20, 85, 30, 150);
const HOVERED: Color32 = Color32::from_rgba_unmultiplied(20, 85, 30, 70);

struct Press {
    start: Pos2,
    last: Pos2,
    spot: Option<Spot>,
    dragging: bool,
    panning: bool,
}

pub(crate) fn spot_test_id(spot: Spot) -> String {
    match spot {
        Spot::Tile { column, row } => format!("game.tile.{column}.{row}"),
        Spot::Pile(pile) => format!("game.pile.{pile}"),
        Spot::Card { pile, card } => format!("game.card.{pile}.{card}"),
    }
}

#[component]
pub(crate) fn Stage(editor: Editor, play: Play, layout: Memo<Layout>, steps: Steps) -> NodeId {
    let canvas = editor.canvas();
    let scale = editor.scale();
    let (annotations, set_annotations) = create_signal(Vec::<Annotation>::new());
    let (pending, set_pending) = create_signal(None::<Annotation>);
    create_effect(clone!(layout set_annotations set_pending -> move || {
        layout.with(|_| {});
        set_annotations.set(Vec::new());
        set_pending.set(None);
    }));

    let world = {
        let (canvas, editor) = (canvas.clone(), editor.clone());
        Rc::new(move |pos: Pos2| {
            canvas
                .get_untracked()
                .unwrap_or_else(|| CanvasView::new(editor.content_rect().min, 1.0))
                .to_canvas(pos)
        })
    };
    let hit = {
        let (layout, world) = (layout.clone(), world.clone());
        Rc::new(move |pos: Pos2| layout.with_untracked(|layout| layout.hit(world(pos))))
    };
    let press = Rc::new(RefCell::new(None::<Press>));
    let finish = {
        let (play, hit) = (play.clone(), hit.clone());
        Rc::new(move |pos: Pos2| {
            let from = play.dragging.get_untracked().map(|dragging| dragging.from);
            play.set_dragging.set(None);
            play.set_over.set(None);
            if let (Some(from), Some(to)) = (from, hit(pos))
                && from != to
            {
                play.dropped(from, to);
            }
        })
    };

    let on_press = clone!(press hit -> move |pointer: PointerPress| {
        *press.borrow_mut() = Some(Press {
            start: pointer.pos,
            last: pointer.pos,
            spot: hit(pointer.pos),
            dragging: false,
            panning: false,
        });
    });
    let on_drag = clone!(press play hit world editor -> move |pointer: PointerPress| {
        let mut held = press.borrow_mut();
        let Some(state) = held.as_mut() else {
            return;
        };
        let delta = pointer.pos - state.last;
        state.last = pointer.pos;
        if state.dragging {
            if let Some(dragging) = play.dragging.get_untracked() {
                play.set_dragging.set(Some(Dragging {
                    at: world(pointer.pos),
                    ..dragging
                }));
            }
            play.set_over.set(hit(pointer.pos));
            return;
        }
        if state.panning {
            editor.pan(delta);
            return;
        }
        if (pointer.pos - state.start).length() < DRAG_THRESHOLD {
            return;
        }
        match state.spot.filter(|spot| play.can_drag(*spot)) {
            Some(from) => {
                state.dragging = true;
                play.set_choices.set(Vec::new());
                play.set_selected.set(Some(from));
                play.set_dragging.set(Some(Dragging {
                    from,
                    at: world(pointer.pos),
                }));
            }
            None => {
                state.panning = true;
                editor.pan(pointer.pos - state.start);
            }
        }
    });
    let on_click_at = clone!(press play hit finish set_annotations -> move |pointer: PointerPress| {
        let state = press.borrow_mut().take();
        set_annotations.set(Vec::new());
        match state {
            Some(state) if state.dragging => finish(pointer.pos),
            Some(state) if state.panning => {}
            _ => match hit(pointer.pos) {
                Some(spot) => play.click(spot),
                None => play.cancel(),
            },
        }
    });
    let on_active_change = clone!(press finish -> move |active: bool| {
        if active {
            return;
        }
        let state = press.borrow_mut().take();
        if let Some(state) = state.filter(|state| state.dragging) {
            finish(state.last);
        }
    });
    let on_secondary_drag = clone!(hit set_annotations set_pending -> move |drag: SecondaryDrag| {
        let Some(from) = hit(drag.from) else {
            set_pending.set(None);
            return;
        };
        let drawn = hit(drag.pos).map(|to| Annotation {
            from,
            to,
            brush: Brush::of(drag.modifiers),
        });
        if drag.cancelled {
            set_pending.set(None);
        } else if drag.ended {
            set_pending.set(None);
            if let Some(drawn) = drawn {
                set_annotations.update(|annotations| *annotations = toggled(annotations, drawn));
            }
        } else {
            set_pending.set(drawn);
        }
    });
    let on_key = clone!(steps -> move |press: KeyPress| {
        let step: fn(&Steps) = match press.key {
            Key::ArrowLeft => Steps::previous,
            Key::ArrowRight => Steps::next,
            Key::Home => Steps::first,
            Key::End => Steps::last,
            _ => return false,
        };
        if press.pressed {
            step(&steps);
        }
        true
    });

    let keys = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.spots.iter().map(|placed| placed.spot).collect::<Vec<_>>())
    }));
    let label_keys = create_memo(clone!(layout -> move || {
        layout.with(|layout| (0..layout.labels.len()).collect::<Vec<_>>())
    }));
    let theme = use_theme();
    let plate = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.plate.unwrap_or(Rect::ZERO))
    }));
    let checkered = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.spots.first().is_some_and(|placed| placed.look == Look::Square))
    }));
    let plate_draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(theme checkered plate -> move || {
        let color = match checkered.get() {
            true => BOARD_EDGE,
            false => theme.get().surface,
        };
        let width = plate.get().width().max(1.0);
        Rc::new(move |painter: &Painter, rect: Rect| {
            painter.rect_filled(rect, PLATE_RADIUS * rect.width() / width, color);
        }) as Draw
    })));
    let bounds = create_memo(clone!(layout -> move || layout.with(Layout::bounds)));
    let marks_draw: Prop<Draw> =
        Prop::Dynamic(Rc::new(clone!(layout annotations pending -> move || {
            let (layout, annotations, pending) = (layout.get(), annotations.get(), pending.get());
            Rc::new(move |painter: &Painter, rect: Rect| {
                annotations::paint(painter, rect, &layout, &annotations, pending);
            }) as Draw
        })));
    let lifted = create_memo(clone!(layout play -> move || {
        let dragging = play.dragging.get()?;
        layout.with(|layout| {
            let placed = layout.find(dragging.from)?;
            let sprite = placed.layers.last()?.clone();
            let size = placed.rect.size();
            Some((Rect::from_center_size(dragging.at, size), sprite))
        })
    }));
    let lifted_rect = create_memo(clone!(lifted -> move || {
        lifted.get().map(|(rect, _)| rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)))
    }));
    let lifted_draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(lifted -> move || {
        let lifted = lifted.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some((_, sprite)) = &lifted {
                paint_sprite(painter, rect, sprite);
            }
        }) as Draw
    })));

    let spots_layout = layout.clone();
    view! {
        <Focusable on_key={on_key}>
            <ClickCatcher
                capture_presses=true
                on_press={on_press}
                on_drag={on_drag}
                on_click_at={on_click_at}
                on_active_change={on_active_change}
                on_secondary_drag={on_secondary_drag}
            >
                <Canvas view={canvas}>
                    <CanvasItem
                        x={create_memo(clone!(plate -> move || plate.get().left()))}
                        y={create_memo(clone!(plate -> move || plate.get().top()))}
                        width={create_memo(clone!(plate -> move || plate.get().width().max(1.0)))}
                        height={create_memo(clone!(plate -> move || plate.get().height().max(1.0)))}
                    >
                        <Drawing draw={plate_draw} />
                    </CanvasItem>
                    <ForEach keys={keys}>
                        {move |spot: Spot| view! {
                            <SpotItem spot layout={spots_layout.clone()} play={play.clone()} />
                        }}
                    </ForEach>
                    <ForEach keys={label_keys}>
                        {move |index: usize| view! {
                            <LabelItem index layout={layout.clone()} scale={scale.clone()} />
                        }}
                    </ForEach>
                    <CanvasItem
                        x=0.0
                        y=0.0
                        width={create_memo(clone!(bounds -> move || bounds.get().width().max(1.0)))}
                        height={create_memo(clone!(bounds -> move || bounds.get().height().max(1.0)))}
                        @test_id={"game.board"}
                    >
                        <Drawing draw={marks_draw} />
                    </CanvasItem>
                    <CanvasItem
                        x={create_memo(clone!(lifted_rect -> move || lifted_rect.get().left()))}
                        y={create_memo(clone!(lifted_rect -> move || lifted_rect.get().top()))}
                        width={create_memo(clone!(lifted_rect -> move || lifted_rect.get().width()))}
                        height={create_memo(clone!(lifted_rect -> move || lifted_rect.get().height()))}
                        @test_id={"game.lifted"}
                    >
                        <Drawing draw={lifted_draw} />
                    </CanvasItem>
                </Canvas>
            </ClickCatcher>
        </Focusable>
    }
}

#[component]
fn SpotItem(spot: Spot, layout: Memo<Layout>, play: Play) -> CanvasItem {
    let placed = create_memo(move || layout.with(|layout| layout.find(spot).cloned()));
    let rect = create_memo(clone!(placed -> move || {
        placed.with(|placed| placed.as_ref().map(|placed| placed.rect)).unwrap_or(Rect::ZERO)
    }));
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(placed -> move || {
        let placed = placed.get();
        let mark = play.mark(spot);
        let hidden = play.dragging.get().is_some_and(|dragging| dragging.from == spot);
        let theme = theme.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some(placed) = &placed {
                paint_spot(painter, rect, placed, mark, hidden, &theme);
            }
        }) as Draw
    })));
    view! {
        <CanvasItem
            x={create_memo(clone!(rect -> move || rect.get().left()))}
            y={create_memo(clone!(rect -> move || rect.get().top()))}
            width={create_memo(clone!(rect -> move || rect.get().width().max(1.0)))}
            height={create_memo(clone!(rect -> move || rect.get().height().max(1.0)))}
            @test_id={spot_test_id(spot)}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn LabelItem(index: usize, layout: Memo<Layout>, scale: ReadSignal<f32>) -> CanvasItem {
    let label = create_memo(move || layout.with(|layout| layout.labels.get(index).cloned()));
    let rect = create_memo(clone!(label -> move || {
        label.with(|label| label.as_ref().map(|label| label.rect)).unwrap_or(Rect::ZERO)
    }));
    let text = create_memo(clone!(label -> move || {
        label.with(|label| label.as_ref().map(|label| label.text.clone())).unwrap_or_default()
    }));
    let font_size = create_memo(move || (LABEL_SIZE * scale.get()).max(1.0));
    let theme = use_theme();
    let color = create_memo(move || theme.get().text_muted);
    view! {
        <CanvasItem
            x={create_memo(clone!(rect -> move || rect.get().left()))}
            y={create_memo(clone!(rect -> move || rect.get().top()))}
            width={create_memo(clone!(rect -> move || rect.get().width().max(1.0)))}
            height={LABEL_HEIGHT}
            @test_id={format!("game.label.{index}")}
        >
            <Text string={text} font_size={font_size} color={color} />
        </CanvasItem>
    }
}

fn paint_spot(
    painter: &Painter,
    rect: Rect,
    placed: &Placed,
    mark: Mark,
    hidden: bool,
    theme: &Theme,
) {
    let scale = rect.width() / placed.rect.width().max(1.0);
    let size = rect.width().min(rect.height());
    match placed.look {
        Look::Cell => {
            let fill = match mark {
                Mark::Target | Mark::Over => theme.accent_soft,
                _ => theme.surface_raised,
            };
            painter.rect_filled(rect, CELL_RADIUS * scale, fill);
        }
        Look::Pile => {
            painter.rect_stroke(
                Rect::from_min_size(rect.min, CARD * scale),
                CARD_RADIUS * scale,
                1.0,
                theme.border,
            );
        }
        Look::Square | Look::Card => {}
    }
    let top = placed.layers.len().saturating_sub(1);
    for (index, layer) in placed.layers.iter().enumerate() {
        let movable = matches!(layer, Sprite::Piece(_) | Sprite::Card(_) | Sprite::CardBack);
        if hidden && index == top && movable {
            continue;
        }
        paint_sprite(painter, rect, layer);
    }
    match placed.look {
        Look::Square => match mark {
            Mark::Selected => painter.rect_filled(rect, 0.0, CHOSEN),
            Mark::Over => painter.rect_filled(rect, 0.0, HOVERED),
            Mark::Target if occupied(placed) => {
                let ring = rect.shrink(size * 0.04);
                painter.rect_stroke(ring, size * 0.46, size * 0.08, LANDING);
            }
            Mark::Target => {
                painter.rect_filled(
                    Rect::from_center_size(rect.center(), Vec2::splat(size * 0.3)),
                    size * 0.15,
                    LANDING,
                );
            }
            Mark::None | Mark::Movable => {}
        },
        Look::Cell => {
            if mark == Mark::Selected {
                painter.rect_stroke(rect, CELL_RADIUS * scale, MARK_WIDTH, theme.warning);
            }
        }
        Look::Card | Look::Pile => {
            let color = match mark {
                Mark::None => return,
                Mark::Selected => theme.warning,
                Mark::Over => theme.accent_active,
                Mark::Movable | Mark::Target => theme.accent,
            };
            painter.rect_stroke(
                rect.expand(MARK_WIDTH / 2.0 + 1.0),
                CARD_RADIUS * scale + 2.0,
                MARK_WIDTH,
                color,
            );
        }
    }
}

fn occupied(placed: &Placed) -> bool {
    placed
        .layers
        .iter()
        .any(|layer| matches!(layer, Sprite::Piece(_)))
}
