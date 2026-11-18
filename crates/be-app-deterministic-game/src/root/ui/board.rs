use be_sdk_beui::beui::CanvasView;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use be_sdk_beui::Editor;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{
    Color32, FontId, Key, KeyPress, NodeId, PointerId, PointerPress, Pos2, Rect,
    SecondaryDrag, TextLayout, Vec2, accesskit, drag, reactive::Painter,
};
use tabletop_api::Spot;
use tabletop_api::board::{CARD_HEIGHT, CARD_WIDTH, HandCard, ItemId, Sprite};

use super::Steps;
use super::annotations::{self, Annotation, Brush, toggled};
use super::hand::{HAND_CARD, HandBar, hand_hit, hand_rects};
use super::layout::{Layout, Look, Placed, SpotPlace, movable};
use super::motion::{Motion, flights, sightings};
use super::play::{Mark, Play};
use super::skin::paint_sprite;

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
    carried: bool,
    panning: bool,
}

/// A piece or card carried from its spot.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Carried(Spot);

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
    let annotations = create_linked_signal(move || {
        layout.track();
        Vec::<Annotation>::new()
    });
    let pending = create_linked_signal(move || {
        layout.track();
        None::<Annotation>
    });
    let bar = create_signal(Rect::ZERO);

    let view_now = move || {
        canvas
            .get_untracked()
            .unwrap_or_else(|| CanvasView::new(editor.content_rect().min, 1.0))
    };
    let world = move |pos: Pos2| view_now().to_canvas(pos);
    let in_bar =
        move |pos: Pos2| !hand.with_untracked(Vec::is_empty) && bar.get_untracked().contains(pos);
    let table_hit = move |pos: Pos2| layout.with_untracked(|layout| layout.hit(world(pos)));
    let hit = move |pos: Pos2| match in_bar(pos) {
        true => hand.with_untracked(|hand| hand_hit(bar.get_untracked(), hand, pos)),
        false => table_hit(pos),
    };

    let motion = Motion::new();
    let dropped = StoredCell::new(None::<(ItemId, Rect, Spot)>);
    create_effect_on(
        move || (drawn.get(), hand.get()),
        move |(now_drawn, now_hand), previous| {
            let dropped = dropped.take().filter(|(id, _, onto)| {
                now_drawn.item(*id).and_then(|placed| placed.spot) == Some(*onto)
            });
            let Some((was_drawn, was_hand)) = previous else {
                return;
            };
            let world = world_size.get_untracked();
            let (view, bar) = (view_now(), bar.get_untracked());
            let clock = motion.clock.get_untracked();
            let mut before = sightings(&was_drawn.centered_in(world), view, was_hand, bar);
            motion.flights.with_untracked(|flying| {
                for seen in &mut before {
                    if let Some(flight) = flying.iter().find(|flight| flight.id == seen.id) {
                        seen.rect = flight.rect(clock);
                    }
                }
            });
            let after: Vec<_> = sightings(&now_drawn.centered_in(world), view, now_hand, bar)
                .into_iter()
                .filter(|seen| movable(&seen.sprite))
                .filter(|seen| bar.height() > 0.0 || now_hand.iter().all(|held| held.id != seen.id))
                .collect();
            let dropped = dropped.map(|(id, rect, _)| (id, rect));
            motion.launch(flights(&before, &after, dropped, now()));
        },
    );

    // The size the item at `from` is drawn at, on the table or in the hand.
    let lifted_size = move |from: Spot| {
        let held = hand.with_untracked(|hand| hand.iter().any(|held| held.spot == from));
        match held {
            true => Some(HAND_CARD),
            false => layout
                .with_untracked(|layout| Some(layout.lifted(from)?.rect.size() * view_now().scale)),
        }
    };
    // Where `spot` is on screen: its hand card, or the union of its items on the table.
    let spot_rect = move |spot: Spot| {
        let in_hand = hand.with_untracked(|hand| {
            hand_rects(bar.get_untracked(), hand.len())
                .into_iter()
                .zip(hand)
                .find(|(_, held)| held.spot == spot)
                .map(|(rect, _)| rect)
        });
        in_hand.or_else(|| {
            let view = view_now();
            layout.with_untracked(|layout| {
                layout
                    .items
                    .iter()
                    .filter(|placed| placed.spot == Some(spot))
                    .map(|placed| view.rect_to_screen(placed.rect))
                    .reduce(|all, rect| all.union(rect))
            })
        })
    };
    // Each move the game offers from the carried spot is a place at the spot it leads to,
    // chosen while the pointer is on it; elsewhere the item floats and a release puts it
    // back.
    let stage = NodeRef::new();
    // Where the item was drawn last, which a drop flies on from: the board's frame is gone
    // by the time the drop commits.
    let last_lifted = StoredCell::new(None::<Lifted>);
    let site = use_drop_site(
        Func::new(|_: Carried| true),
        Func::new(move |Carried(from): Carried| {
            let placed = stage
                .try_get()
                .and_then(|node| measure(node).get_untracked());
            let (Some(placed), Some(size)) = (placed, lifted_size(from)) else {
                return drag::floating();
            };
            let origin = placed.rect;
            let scale = match placed.size.y > 0.0 {
                true => origin.height() / placed.size.y,
                false => 1.0,
            };
            let local = |rect: Rect| {
                Rect::from_min_size(
                    Pos2::ZERO + (rect.min - origin.min) / scale,
                    rect.size() / scale,
                )
            };
            let targets = play.targets(from).into_iter().filter_map(|to| {
                let zone = local(spot_rect(to)?);
                let rest = Rect::from_center_size(zone.center(), size / scale);
                Some(drag::Candidate::new(to, rest, stage).zoned(zone))
            });
            drag::closest(targets.collect::<Vec<_>>()).when_far(drag::floating())
        }),
        Callback::new(move |(Carried(from), to): (Carried, Spot)| {
            let carried = last_lifted.take();
            dropped.set(carried.map(|carried| (carried.id, carried.rect, to)));
            play.dropped(from, to);
        }),
    );
    let play = Play {
        over: site.over,
        ..play
    };
    let board = beui::unstyled::drag_board();
    let lifted = create_memo(move || {
        // The board's site serves one carry at a time: the first lifted.
        let carried = carrying::<Carried>().into_iter().next()?;
        let rect = board.frame_of(&carried)?.item;
        let Carried(from) = carried;
        let held = hand.with(|hand| {
            hand.iter()
                .find(|held| held.spot == from)
                .map(|held| (held.id, held.sprite.clone()))
        });
        let (id, sprite) = held.or_else(|| {
            layout.with(|layout| {
                let placed = layout.lifted(from)?;
                Some((placed.id, placed.sprite.clone()))
            })
        })?;
        Some(Lifted { id, sprite, rect })
    });
    create_effect(move || {
        if let Some(lifted) = lifted.get() {
            last_lifted.set(Some(lifted));
        }
    });
    let hidden = create_memo(move || {
        let mut hidden = motion.flying.get();
        hidden.extend(lifted.with(|lifted| lifted.as_ref().map(|lifted| lifted.id)));
        hidden
    });

    let press = store(RefCell::new(None::<Press>));
    let on_press = move |pointer: PointerPress| {
        *press.value().borrow_mut() = Some(Press {
            start: pointer.pos,
            last: pointer.pos,
            spot: hit(pointer.pos),
            carried: false,
            panning: false,
        });
    };
    // A press on a piece that can move lifts it; any other press goes on as a pan. The
    // keyboard lifts the piece selected.
    // The carry of the piece at `from`, selecting it: a press on it lifts it, as does the
    // keyboard on the stage (the piece selected) or on the piece itself (a focus stop).
    let lift_from = Func::new(move |from: Spot| {
        if !play.can_drag(from) {
            return None;
        }
        let size = lifted_size(from)?;
        play.choices.set(Vec::new());
        play.selected.set(Some(from));
        Some(
            drag::Carry::new(Carried(from))
                .item(Rect::from_center_size(Pos2::ZERO, size))
                .drawn_by_site(),
        )
    });
    let drag = Func::new(move |pointer: PointerPress| {
        let from = match pointer.pointer {
            PointerId::Keyboard => play.selected.get_untracked()?,
            _ => {
                let pressed = press.value();
                let mut held = pressed.borrow_mut();
                let state = held.as_mut()?;
                let from = state.spot.filter(|spot| play.can_drag(*spot))?;
                state.carried = true;
                from
            }
        };
        lift_from.call(from)
    });
    let on_drag = move |pointer: PointerPress| {
        let pressed = press.value();
        let mut held = pressed.borrow_mut();
        let Some(state) = held.as_mut() else {
            return;
        };
        let delta = pointer.pos - state.last;
        state.last = pointer.pos;
        if state.panning {
            editor.pan(delta);
        } else if !in_bar(state.start) {
            state.panning = true;
            editor.pan(pointer.pos - state.start);
            editor.pan_drag();
        }
    };
    let on_click_at = move |pointer: PointerPress| {
        let state = press.value().borrow_mut().take();
        annotations.set(Vec::new());
        match state {
            Some(state) if state.carried || state.panning => {}
            _ => match hit(pointer.pos) {
                Some(spot) => play.click(spot),
                None => play.cancel(),
            },
        }
    };
    let on_active_change = move |active: bool| {
        if !active {
            press.value().borrow_mut().take();
        }
    };
    let on_secondary_drag = move |drag: SecondaryDrag| {
        let Some(from) = table_hit(drag.from) else {
            pending.set(None);
            return;
        };
        let drawn = table_hit(drag.pos).map(|to| Annotation {
            from,
            to,
            brush: Brush::of(drag.modifiers),
        });
        if drag.cancelled {
            pending.set(None);
        } else if drag.ended {
            pending.set(None);
            if let Some(drawn) = drawn {
                annotations.update(|annotations| *annotations = toggled(annotations, drawn));
            }
        } else {
            pending.set(drawn);
        }
    };
    let on_key = move |press: KeyPress| {
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
    };

    // One map for every item to find whether it is on top, rather than each searching.
    let faces = create_memo(move || layout.with(Layout::faces));
    let item_keys = create_memo(move || {
        layout.with(|layout| {
            layout
                .items
                .iter()
                .map(|placed| placed.id)
                .collect::<Vec<_>>()
        })
    });
    let spot_keys = create_memo(move || {
        layout.with(|layout| {
            layout
                .spots
                .iter()
                .map(|place| place.spot)
                .collect::<Vec<_>>()
        })
    });
    let bounds = create_memo(move || layout.with(Layout::bounds));
    let marks_draw = move || {
        let (layout, annotations, pending) = (layout.get(), annotations.get(), pending.get());
        Rc::new(move |painter: &Painter, rect: Rect| {
            annotations::paint(painter, rect, &layout, &annotations, pending);
        }) as Draw
    };
    let lifted_rect = create_memo(move || {
        lifted
            .with(|lifted| lifted.as_ref().map(|lifted| lifted.rect))
            .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)))
    });
    let lifted_draw = move || {
        let lifted = lifted.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some(lifted) = &lifted {
                paint_sprite(painter, rect, &lifted.sprite);
            }
        }) as Draw
    };

    // The stage's presses are caught by the `Interactive` around the table.
    let built = view! {
        <Interactive
                focusable=true
                on_key={on_key}
                capture_presses=true
                on_press={on_press}
                drag={drag}
                on_drag={on_drag}
                on_click_at={on_click_at}
                on_active_change={on_active_change}
                on_pointer={beui::unstyled::gestures::secondary_drag(on_secondary_drag)}
            >
            <Layers @node_ref=&stage>
                <List spacing=Space::None>
                    <ListItem grow=1.0>
                        <Canvas view={canvas} @node_ref={&content}>
                            <ForEach value={item_keys}>|id: ItemId|
                                <TableItem id layout={layout} faces play={play} hidden={hidden} lift_from />
                            </ForEach>
                            <ForEach value={spot_keys}>|spot: Spot|
                                <SpotItem spot layout={layout} play={play} />
                            </ForEach>
                            <CanvasItem
                                x=0.0
                                y=0.0
                                width={move || bounds.get().width().max(1.0)}
                                height={move || bounds.get().height().max(1.0)}
                                @test_id={"game.board"}
                            >
                                <Drawing draw={marks_draw} />
                            </CanvasItem>
                        </Canvas>
                    </ListItem>
                    <HandBar hand bar play={play} hidden />
                </List>
                <Canvas view={screen_view()}>
                    <ForEach value={motion.flying}>|id: ItemId|
                        <FlightItem id motion={motion} />
                    </ForEach>
                    <CanvasItem
                        x={create_memo(move || lifted_rect.get().left())}
                        y={create_memo(move || lifted_rect.get().top())}
                        width={create_memo(move || lifted_rect.get().width())}
                        height={create_memo(move || lifted_rect.get().height())}
                        clip=false
                        @test_id={"game.lifted"}
                    >
                        <Drawing draw={lifted_draw} />
                    </CanvasItem>
                </Canvas>
            </Layers>
        </Interactive>
    };
    // Measured from the start, so a lift reads where the stage is.
    if let Some(node) = stage.try_get() {
        measure(node);
    }
    built
}

#[component]
fn TableItem(
    id: ItemId,
    layout: Memo<Layout>,
    faces: Memo<HashMap<Spot, ItemId>>,
    play: Play,
    hidden: Memo<Vec<ItemId>>,
    lift_from: Func<Spot, Option<drag::Carry>>,
) -> CanvasItem {
    let placed = create_memo(move || {
        layout.with(|layout| layout.item(id).cloned().map(|placed| (placed, layout.unit)))
    });
    let rect = create_memo(move || {
        placed
            .with(|placed| placed.as_ref().map(|(placed, _)| placed.rect))
            .unwrap_or(Rect::ZERO)
    });
    let theme = use_theme();
    let draw = move || {
        let placed = placed.get();
        let mark = match &placed {
            Some((
                Placed {
                    sprite: Sprite::Cell,
                    spot: Some(spot),
                    ..
                },
                _,
            )) => play.mark(*spot),
            _ => Mark::None,
        };
        let hidden = hidden.with(|hidden| hidden.contains(&id));
        let theme = theme.get().resolve();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some((placed, unit)) = &placed
                && !hidden
            {
                let zoom = rect.width() / placed.rect.width().max(1.0) * unit;
                paint_item(painter, rect, placed, zoom, mark, &theme);
            }
        }) as Draw
    };
    let drawing = view! {
        <Drawing draw={draw} />
    };
    // Untracked: an item never turns from a piece or card into the board's own sprites, or
    // back, so whether it is a stop is settled once; its spot and face are tracked within.
    let piece = placed.with_untracked(|placed| {
        placed
            .as_ref()
            .is_some_and(|(placed, _)| piece_name(&placed.sprite).is_some())
    });
    // Only the one on top of a pile is its spot's stop, and read: the rest lie under it.
    let on_top = create_memo(move || {
        placed
            .with(|placed| placed.as_ref().and_then(|(placed, _)| placed.spot))
            .is_none_or(|spot| faces.with(|faces| faces.get(&spot) == Some(&id)))
    });
    let face = match piece {
        true => view! {
            <Frame accessibility_hidden={move || !on_top.get()}>
                <PieceStop id layout on_top play lift_from>{drawing}</PieceStop>
            </Frame>
        },
        false => drawing,
    };
    view! {
        <CanvasItem
            x={move || rect.get().left()}
            y={move || rect.get().top()}
            width={move || rect.get().width().max(1.0)}
            height={move || rect.get().height().max(1.0)}
        >
            {face}
        </CanvasItem>
    }
}

/// A piece or card on the table, as a focus stop while it can move: Tab and Shift+Tab
/// reach the movable ones in board order, one for each spot (`on_top`: the top card of a
/// pile, which a press would take), and Space or Enter lifts the focused one, which the keyboard then
/// carries as any other (guides/beui_keyboard.md). Its presses are the stage's, which
/// catches them around the table; a press focuses it, so Space lifts what it selected.
#[component]
fn PieceStop(
    id: ItemId,
    layout: Memo<Layout>,
    on_top: Memo<bool>,
    play: Play,
    lift_from: Func<Spot, Option<drag::Carry>>,
    children: Child,
) -> NodeId {
    // Where it stands and what it is, which a move or a turned card changes.
    let named = create_memo(move || {
        layout.with(|layout| {
            let placed = layout.item(id)?;
            Some((placed.spot, piece_name(&placed.sprite)?))
        })
    });
    let spot = create_memo(move || named.with(|named| named.as_ref().and_then(|(spot, _)| *spot)));
    let movable =
        create_memo(move || on_top.get() && spot.get().is_some_and(|spot| play.movable(spot)));
    let focused = create_signal(false);
    component_accessibility(create_memo(move || {
        let mut node = accesskit::Node::new(accesskit::Role::Button);
        named.with(|named| {
            if let Some((spot, name)) = named {
                node.set_label(match spot {
                    Some(spot) => format!("{name}, {}", square_name(*spot)),
                    None => name.clone(),
                });
            }
        });
        if !movable.get() {
            node.set_disabled();
        }
        node
    }));
    view! {
        <Interactive
            @test_id={move || match (spot.get(), on_top.get()) {
                (Some(spot), true) => piece_test_id(spot),
                _ => format!("game.piece.under.{}", id.0),
            }}
            focusable=true
            tab_stop={movable}
            on_focus_change={focused}
            drag={Func::new(move |pointer: PointerPress| match (pointer.pointer, spot.get_untracked()) {
                (PointerId::Keyboard, Some(spot)) => lift_from.call(spot),
                _ => None,
            })}
        >
            <FocusRing focused>{children}</FocusRing>
        </Interactive>
    }
}

/// What a sprite on the table is called, for a screen reader: a piece by its kind and
/// seat, a card by its rank and suit. Nothing for the board's own frames and labels.
fn piece_name(sprite: &Sprite) -> Option<String> {
    match sprite {
        Sprite::Piece(piece) => Some(format!("Player {}'s {}", piece.seat + 1, piece.kind)),
        Sprite::Card(card) => Some(format!("{:?} of {:?}", card.rank, card.suit)),
        Sprite::CardBack => Some("Card".to_owned()),
        _ => None,
    }
}

fn piece_test_id(spot: Spot) -> String {
    match spot {
        Spot::Tile { column, row } => format!("game.piece.{column}.{row}"),
        Spot::Pile(pile) => format!("game.piece.pile.{pile}"),
        Spot::Card { pile, card } => format!("game.piece.{pile}.card.{card}"),
    }
}

/// Where a spot is, in words.
fn square_name(spot: Spot) -> String {
    match spot {
        Spot::Tile { column, row } => format!("column {}, row {}", column + 1, row + 1),
        Spot::Pile(pile) => format!("pile {}", pile + 1),
        Spot::Card { pile, card } => format!("pile {}, card {}", pile + 1, card + 1),
    }
}

#[component]
fn SpotItem(spot: Spot, layout: Memo<Layout>, play: Play) -> CanvasItem {
    let place = create_memo(move || {
        layout.with(|layout| layout.find(spot).cloned().map(|place| (place, layout.unit)))
    });
    let rect = create_memo(move || {
        place
            .with(|place| place.as_ref().map(|(place, _)| place.rect))
            .unwrap_or(Rect::ZERO)
    });
    let theme = use_theme();
    let draw = move || {
        let place = place.get();
        let mark = play.mark(spot);
        let theme = theme.get().resolve();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some((place, unit)) = &place {
                let zoom = rect.width() / place.rect.width().max(1.0) * unit;
                paint_spot(painter, rect, place, mark, zoom, &theme);
            }
        }) as Draw
    };
    view! {
        <CanvasItem
            x={move || rect.get().left()}
            y={move || rect.get().top()}
            width={move || rect.get().width().max(1.0)}
            height={move || rect.get().height().max(1.0)}
            clip=false
            @test_id={spot_test_id(spot)}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn FlightItem(id: ItemId, motion: Motion) -> CanvasItem {
    let flight = create_memo(move || {
        motion
            .flights
            .with(|flights| flights.iter().find(|flight| flight.id == id).cloned())
    });
    let rect = create_memo(move || {
        let now = motion.clock.get();
        flight
            .with(|flight| flight.as_ref().map(|flight| flight.rect(now)))
            .unwrap_or(Rect::ZERO)
    });
    let draw = move || {
        let flight = flight.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some(flight) = &flight {
                paint_sprite(painter, rect, &flight.sprite);
            }
        }) as Draw
    };
    view! {
        <CanvasItem
            x={move || rect.get().left()}
            y={move || rect.get().top()}
            width={move || rect.get().width().max(1.0)}
            height={move || rect.get().height().max(1.0)}
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
                _ => theme.control,
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
            let label = painter.lay_out_text(text, FontId::proportional(size), TextLayout::DEFAULT);
            let origin = rect.min + Vec2::new(0.0, (rect.height() - label.size().y) / 2.0);
            painter.draw_text(origin, &label, theme.text_muted);
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
