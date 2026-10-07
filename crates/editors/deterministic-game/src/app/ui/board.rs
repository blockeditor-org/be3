use std::cell::{Cell, RefCell};
use std::rc::Rc;

use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::{
    Canvas, CanvasItem, CanvasView, Draw, Drawing, ForEach, Interactive, ItemSize, Layers, List,
    Memo, NodeRef, Prop, clone, component, create_effect, create_memo, create_signal, now, view,
    with_document,
};
use block_editor_beui::beui::styled::{Theme, use_theme};
use block_editor_beui::beui::{
    Color32, FontId, Key, KeyPress, NodeId, Painter, PointerPress, Pos2, Rect, SecondaryDrag, Vec2,
};
use game_api::Spot;
use game_api::board::{CARD_HEIGHT, CARD_WIDTH, HandCard, ItemId, Sprite};

use super::Steps;
use super::annotations::{self, Annotation, Brush, toggled};
use super::hand::{HAND_CARD, HandBar, hand_hit};
use super::layout::{Layout, Look, Placed, SpotPlace, movable};
use super::motion::{Motion, flights, sightings};
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

#[derive(Clone, Debug, PartialEq)]
struct Lifted {
    id: ItemId,
    sprite: Sprite,
    rect: Rect,
}

pub(crate) fn spot_test_id(spot: Spot) -> String {
    match spot {
        Spot::Tile { column, row } => format!("game.tile.{column}.{row}"),
        Spot::Pile(pile) => format!("game.pile.{pile}"),
        Spot::Card { pile, card } => format!("game.card.{pile}.{card}"),
    }
}

fn screen_view() -> Option<CanvasView> {
    Some(CanvasView::new(Pos2::ZERO, 1.0))
}

#[component]
pub(crate) fn Stage(
    editor: Editor,
    play: Play,
    drawn: Memo<Layout>,
    layout: Memo<Layout>,
    hand: Memo<Vec<HandCard>>,
    steps: Steps,
    content: NodeRef,
) -> NodeId {
    let canvas = editor.canvas();
    let world_size = editor.world();
    let (annotations, set_annotations) = create_signal(Vec::<Annotation>::new());
    let (pending, set_pending) = create_signal(None::<Annotation>);
    create_effect(clone!(layout set_annotations set_pending -> move || {
        layout.with(|_| {});
        set_annotations.set(Vec::new());
        set_pending.set(None);
    }));
    let (bar, set_bar) = create_signal(Rect::ZERO);

    let view_now = {
        let (canvas, editor) = (canvas.clone(), editor.clone());
        Rc::new(move || {
            canvas
                .get_untracked()
                .unwrap_or_else(|| CanvasView::new(editor.content_rect().min, 1.0))
        })
    };
    let world = {
        let view_now = view_now.clone();
        Rc::new(move |pos: Pos2| view_now().to_canvas(pos))
    };
    let in_bar = {
        let (hand, bar) = (hand.clone(), bar.clone());
        Rc::new(move |pos: Pos2| {
            !hand.with_untracked(Vec::is_empty) && bar.get_untracked().contains(pos)
        })
    };
    let table_hit = {
        let (layout, world) = (layout.clone(), world.clone());
        Rc::new(move |pos: Pos2| layout.with_untracked(|layout| layout.hit(world(pos))))
    };
    let hit = {
        let (hand, bar, in_bar, table_hit) =
            (hand.clone(), bar.clone(), in_bar.clone(), table_hit.clone());
        Rc::new(move |pos: Pos2| match in_bar(pos) {
            true => hand.with_untracked(|hand| hand_hit(bar.get_untracked(), hand, pos)),
            false => table_hit(pos),
        })
    };

    let motion = Motion::new();
    let dropped = Rc::new(Cell::new(None::<(ItemId, Rect, Spot)>));
    let previous = Rc::new(RefCell::new(None::<(Layout, Vec<HandCard>)>));
    create_effect(
        clone!(drawn hand motion dropped previous view_now bar world_size -> move || {
            let now_drawn = drawn.get();
            let now_hand = hand.get();
            let earlier = previous.replace(Some((now_drawn.clone(), now_hand.clone())));
            let dropped = dropped.take().filter(|(id, _, onto)| {
                now_drawn.item(*id).and_then(|placed| placed.spot) == Some(*onto)
            });
            let Some((was_drawn, was_hand)) = earlier else {
                return;
            };
            let world = world_size.get_untracked();
            let (view, bar) = (view_now(), bar.get_untracked());
            let clock = motion.clock.get_untracked();
            let mut before = sightings(&was_drawn.centered_in(world), view, &was_hand, bar);
            motion.flights.with_untracked(|flying| {
                for seen in &mut before {
                    if let Some(flight) = flying.iter().find(|flight| flight.id == seen.id) {
                        seen.rect = flight.rect(clock);
                    }
                }
            });
            let after: Vec<_> = sightings(&now_drawn.centered_in(world), view, &now_hand, bar)
                .into_iter()
                .filter(|seen| movable(&seen.sprite))
                .filter(|seen| bar.height() > 0.0 || now_hand.iter().all(|held| held.id != seen.id))
                .collect();
            let dropped = dropped.map(|(id, rect, _)| (id, rect));
            motion.launch(flights(&before, &after, dropped, now()));
        }),
    );

    let lifted = create_memo(clone!(play layout hand view_now -> move || {
        let dragging = play.dragging.get()?;
        let held = hand.with(|hand| {
            hand.iter()
                .find(|held| held.spot == dragging.from)
                .map(|held| (held.id, held.sprite.clone(), HAND_CARD))
        });
        let (id, sprite, size) = held.or_else(|| {
            layout.with(|layout| {
                let placed = layout.lifted(dragging.from)?;
                let size = placed.rect.size() * view_now().scale;
                Some((placed.id, placed.sprite.clone(), size))
            })
        })?;
        Some(Lifted {
            id,
            sprite,
            rect: Rect::from_center_size(dragging.at, size),
        })
    }));
    let hidden = create_memo(clone!(lifted motion -> move || {
        let mut hidden = motion.flying.get();
        hidden.extend(lifted.with(|lifted| lifted.as_ref().map(|lifted| lifted.id)));
        hidden
    }));

    let press = Rc::new(RefCell::new(None::<Press>));
    let finish = {
        let (play, hit, lifted, dropped) =
            (play.clone(), hit.clone(), lifted.clone(), dropped.clone());
        Rc::new(move |pos: Pos2| {
            let from = play.dragging.get_untracked().map(|dragging| dragging.from);
            let carried = lifted.get_untracked();
            play.set_dragging.set(None);
            play.set_over.set(None);
            if let (Some(from), Some(to)) = (from, hit(pos))
                && from != to
            {
                dropped.set(carried.map(|carried| (carried.id, carried.rect, to)));
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
    let on_drag = clone!(press play hit in_bar editor -> move |pointer: PointerPress| {
        let mut held = press.borrow_mut();
        let Some(state) = held.as_mut() else {
            return;
        };
        let delta = pointer.pos - state.last;
        state.last = pointer.pos;
        if state.dragging {
            if !with_document(|document| document.motion().follows_gestures()) {
                return;
            }
            if let Some(dragging) = play.dragging.get_untracked() {
                play.set_dragging.set(Some(Dragging {
                    at: pointer.pos,
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
                    at: pointer.pos,
                }));
            }
            None if in_bar(state.start) => {}
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
    let on_secondary_drag = clone!(table_hit set_annotations set_pending -> move |drag: SecondaryDrag| {
        let Some(from) = table_hit(drag.from) else {
            set_pending.set(None);
            return;
        };
        let drawn = table_hit(drag.pos).map(|to| Annotation {
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

    let item_keys = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.items.iter().map(|placed| placed.id).collect::<Vec<_>>())
    }));
    let spot_keys = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.spots.iter().map(|place| place.spot).collect::<Vec<_>>())
    }));
    let bounds = create_memo(clone!(layout -> move || layout.with(Layout::bounds)));
    let marks_draw: Prop<Draw> =
        Prop::Dynamic(Rc::new(clone!(layout annotations pending -> move || {
            let (layout, annotations, pending) = (layout.get(), annotations.get(), pending.get());
            Rc::new(move |painter: &Painter, rect: Rect| {
                annotations::paint(painter, rect, &layout, &annotations, pending);
            }) as Draw
        })));
    let lifted_rect = create_memo(clone!(lifted -> move || {
        lifted
            .with(|lifted| lifted.as_ref().map(|lifted| lifted.rect))
            .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)))
    }));
    let lifted_draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(lifted -> move || {
        let lifted = lifted.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some(lifted) = &lifted {
                paint_sprite(painter, rect, &lifted.sprite);
            }
        }) as Draw
    })));
    let flight_keys = create_memo(clone!(motion -> move || motion.flying.get()));

    let (items_layout, items_play, items_hidden) = (layout.clone(), play.clone(), hidden.clone());
    let (spots_layout, spots_play) = (layout.clone(), play.clone());
    let bar_play = play.clone();
    view! {
        <Interactive
            focusable=true
            on_key={on_key}
            capture_presses=true
            on_press={on_press}
            on_drag={on_drag}
            on_click_at={on_click_at}
            on_active_change={on_active_change}
            on_secondary_drag={on_secondary_drag}
        >
            <Layers>
                <List spacing=0.0>
                    <Canvas view={canvas} @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                        <ForEach keys={item_keys}>
                            {move |id: ItemId| view! {
                                <TableItem
                                    id
                                    layout={items_layout.clone()}
                                    play={items_play.clone()}
                                    hidden={items_hidden.clone()}
                                />
                            }}
                        </ForEach>
                        <ForEach keys={spot_keys}>
                            {move |spot: Spot| view! {
                                <SpotItem
                                    spot
                                    layout={spots_layout.clone()}
                                    play={spots_play.clone()}
                                />
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
                    </Canvas>
                    <HandBar hand set_bar play={bar_play} hidden />
                </List>
                <Canvas view={screen_view()}>
                    <ForEach keys={flight_keys}>
                        {move |id: ItemId| view! {
                            <FlightItem id motion={motion.clone()} />
                        }}
                    </ForEach>
                    <CanvasItem
                        x={create_memo(clone!(lifted_rect -> move || lifted_rect.get().left()))}
                        y={create_memo(clone!(lifted_rect -> move || lifted_rect.get().top()))}
                        width={create_memo(clone!(lifted_rect -> move || lifted_rect.get().width()))}
                        height={create_memo(clone!(lifted_rect -> move || lifted_rect.get().height()))}
                        clip=false
                        @test_id={"game.lifted"}
                    >
                        <Drawing draw={lifted_draw} />
                    </CanvasItem>
                </Canvas>
            </Layers>
        </Interactive>
    }
}

#[component]
fn TableItem(
    id: ItemId,
    layout: Memo<Layout>,
    play: Play,
    hidden: Memo<Vec<ItemId>>,
) -> CanvasItem {
    let placed = create_memo(move || {
        layout.with(|layout| layout.item(id).cloned().map(|placed| (placed, layout.unit)))
    });
    let rect = create_memo(clone!(placed -> move || {
        placed.with(|placed| placed.as_ref().map(|(placed, _)| placed.rect)).unwrap_or(Rect::ZERO)
    }));
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(placed -> move || {
        let placed = placed.get();
        let mark = match &placed {
            Some((Placed { sprite: Sprite::Cell, spot: Some(spot), .. }, _)) => play.mark(*spot),
            _ => Mark::None,
        };
        let hidden = hidden.with(|hidden| hidden.contains(&id));
        let theme = theme.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some((placed, unit)) = &placed
                && !hidden
            {
                let zoom = rect.width() / placed.rect.width().max(1.0) * unit;
                paint_item(painter, rect, placed, zoom, mark, &theme);
            }
        }) as Draw
    })));
    view! {
        <CanvasItem
            x={create_memo(clone!(rect -> move || rect.get().left()))}
            y={create_memo(clone!(rect -> move || rect.get().top()))}
            width={create_memo(clone!(rect -> move || rect.get().width().max(1.0)))}
            height={create_memo(clone!(rect -> move || rect.get().height().max(1.0)))}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn SpotItem(spot: Spot, layout: Memo<Layout>, play: Play) -> CanvasItem {
    let place = create_memo(move || {
        layout.with(|layout| layout.find(spot).cloned().map(|place| (place, layout.unit)))
    });
    let rect = create_memo(clone!(place -> move || {
        place.with(|place| place.as_ref().map(|(place, _)| place.rect)).unwrap_or(Rect::ZERO)
    }));
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(place -> move || {
        let place = place.get();
        let mark = play.mark(spot);
        let theme = theme.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some((place, unit)) = &place {
                let zoom = rect.width() / place.rect.width().max(1.0) * unit;
                paint_spot(painter, rect, place, mark, zoom, &theme);
            }
        }) as Draw
    })));
    view! {
        <CanvasItem
            x={create_memo(clone!(rect -> move || rect.get().left()))}
            y={create_memo(clone!(rect -> move || rect.get().top()))}
            width={create_memo(clone!(rect -> move || rect.get().width().max(1.0)))}
            height={create_memo(clone!(rect -> move || rect.get().height().max(1.0)))}
            clip=false
            @test_id={spot_test_id(spot)}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn FlightItem(id: ItemId, motion: Motion) -> CanvasItem {
    let flight = create_memo(clone!(motion -> move || {
        motion.flights.with(|flights| flights.iter().find(|flight| flight.id == id).cloned())
    }));
    let rect = create_memo(clone!(flight -> move || {
        let now = motion.clock.get();
        flight
            .with(|flight| flight.as_ref().map(|flight| flight.rect(now)))
            .unwrap_or(Rect::ZERO)
    }));
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(flight -> move || {
        let flight = flight.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some(flight) = &flight {
                paint_sprite(painter, rect, &flight.sprite);
            }
        }) as Draw
    })));
    view! {
        <CanvasItem
            x={create_memo(clone!(rect -> move || rect.get().left()))}
            y={create_memo(clone!(rect -> move || rect.get().top()))}
            width={create_memo(clone!(rect -> move || rect.get().width().max(1.0)))}
            height={create_memo(clone!(rect -> move || rect.get().height().max(1.0)))}
            clip=false
            @test_id={format!("game.flight.{}", id.0)}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

fn paint_item(
    painter: &Painter,
    rect: Rect,
    placed: &Placed,
    zoom: f32,
    mark: Mark,
    theme: &Theme,
) {
    match &placed.sprite {
        Sprite::Frame => painter.rect_filled(rect, PLATE_RADIUS * zoom, BOARD_EDGE),
        Sprite::Tray => painter.rect_filled(rect, PLATE_RADIUS * zoom, theme.surface),
        Sprite::Cell => {
            let fill = match mark {
                Mark::Target | Mark::Over => theme.accent_soft,
                _ => theme.surface_raised,
            };
            painter.rect_filled(rect, CELL_RADIUS * zoom, fill);
        }
        Sprite::Slot => {
            let card = Vec2::new(CARD_WIDTH as f32, CARD_HEIGHT as f32) * zoom;
            painter.rect_stroke(
                Rect::from_min_size(rect.min, card),
                CARD_RADIUS * zoom,
                1.0,
                theme.border,
            );
        }
        Sprite::Label(text) => {
            let size = (LABEL_SIZE * zoom).max(1.0);
            let galley = painter.layout(text, FontId::proportional(size), f32::INFINITY);
            let origin = rect.min + Vec2::new(0.0, (rect.height() - galley.size().y) / 2.0);
            painter.galley(origin, galley, theme.text_muted);
        }
        sprite => paint_sprite(painter, rect, sprite),
    }
}

pub(crate) fn paint_card_mark(painter: &Painter, rect: Rect, mark: Mark, zoom: f32, theme: &Theme) {
    let color = match mark {
        Mark::None => return,
        Mark::Selected => theme.warning,
        Mark::Over => theme.accent_active,
        Mark::Movable | Mark::Target => theme.accent,
    };
    painter.rect_stroke(
        rect.expand(MARK_WIDTH / 2.0 + 1.0),
        CARD_RADIUS * zoom + 2.0,
        MARK_WIDTH,
        color,
    );
}

fn paint_spot(
    painter: &Painter,
    rect: Rect,
    place: &SpotPlace,
    mark: Mark,
    zoom: f32,
    theme: &Theme,
) {
    let size = rect.width().min(rect.height());
    match place.look {
        Look::Square => match mark {
            Mark::Selected => painter.rect_filled(rect, 0.0, CHOSEN),
            Mark::Over => painter.rect_filled(rect, 0.0, HOVERED),
            Mark::Target if place.occupied => {
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
                painter.rect_stroke(rect, CELL_RADIUS * zoom, MARK_WIDTH, theme.warning);
            }
        }
        Look::Card => paint_card_mark(painter, rect, mark, zoom, theme),
        Look::Plain => {}
    }
}
