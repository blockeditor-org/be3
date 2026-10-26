use std::cell::Cell;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use beui::icons::{
    ICON_CHEVRON_DOWN, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CHEVRON_UP, ICON_CHEVRONS_DOWN,
    ICON_CHEVRONS_LEFT, ICON_CHEVRONS_RIGHT, ICON_CHEVRONS_UP, ICON_PAUSE, ICON_PLAY,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{self, RemoteTextCursor, TextAreaState, narrower_than};
use beui::{Color32, Direction, ItemSize, NodeId};
use beui_macros::{component, sample};
use text_editor_core::schema::basic::{self, Basic};
use text_editor_core::{CursorPosition, EditorCommand, TextLanguage, Value};

use super::{Caret, Side, SideDocument, Simulation};
use crate::rich_text::{GLYPHS, render_marker};
use crate::sample::{Sample, ScrollPage};
use crate::{ROW_SPACING, SECTION_SPACING};

const TEXT: &str = "# Shared notes\n\nType on both sides. Each side keeps its own copy and \
sends its edits over a simulated network.\n\n- [ ] type at the same place on both sides\n";
const RICH_TEXT: &str = "Shared notes\nType on both sides and style what you select. Bold, \
italic and code travel as marks that each side merges.\nPause the network, select the same \
words on both sides and style them differently.\n";
const EDITOR_HEIGHT: f32 = 240.0;
const MAX_LATENCY: f32 = 2000.0;
const START_LATENCY: f32 = 300.0;
const COLUMNS_BREAKPOINT: f32 = 640.0;
const LEFT_COLOR: Color32 = Color32::from_rgb(76, 140, 255);
const RIGHT_COLOR: Color32 = Color32::from_rgb(240, 140, 60);

#[component]
pub(crate) fn CollaborationPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Two editors on one sequence"
                code={vec![ConcurrentEditing::SOURCE, SendButtons::SOURCE, EditorPane::SOURCE]}
            >
                <ConcurrentEditing rich=false />
            </Sample>
        </ScrollPage>
    }
}

pub(crate) fn rich_simulation() -> Simulation {
    let words = |words: &str| {
        let start = RICH_TEXT.find(words).unwrap_or(0);
        start..start + words.len()
    };
    Simulation::rich(
        RICH_TEXT,
        &[
            basic::block(words("Shared notes"), basic::HEADING, Value::Int(1)),
            basic::mark(words("Bold"), basic::BOLD),
            basic::mark(words("italic"), basic::ITALIC),
            basic::mark(words("code"), basic::CODE),
            basic::mark(words("differently"), basic::UNDERLINE),
            basic::block(
                words("Pause the network"),
                basic::LIST,
                Value::text(basic::CHECK),
            ),
        ],
    )
}

fn editor(simulation: &Arc<RwLock<Simulation>>, side: Side, rich: bool) -> TextAreaState {
    let document = Arc::new(SideDocument::new(simulation, side));
    if rich {
        return TextAreaState::with_schema(document, Arc::new(Basic));
    }
    let state = TextAreaState::new(document);
    state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
    state
}

fn caret(state: &TextAreaState) -> Option<Caret> {
    state.with_core(|core| {
        core.cursor_positions().first().map(|cursor| Caret {
            anchor: cursor.pos.anchor,
            focus: cursor.pos.focus,
        })
    })
}

fn remote(state: &TextAreaState, caret: Option<Caret>, color: Color32) -> Vec<RemoteTextCursor> {
    state.with_core(|core| {
        caret
            .and_then(|caret| {
                Some(RemoteTextCursor {
                    selection: core
                        .selection_range(&CursorPosition::range(caret.anchor, caret.focus))?,
                    caret: core.position_index(caret.focus)?,
                    color,
                })
            })
            .into_iter()
            .collect()
    })
}

#[sample]
#[component]
pub(crate) fn ConcurrentEditing(rich: bool) -> NodeId {
    let simulation = Arc::new(RwLock::new(match rich {
        true => rich_simulation(),
        false => Simulation::new(TEXT),
    }));
    let left = editor(&simulation, Side::Left, rich);
    let right = editor(&simulation, Side::Right, rich);
    let paused = create_signal(false);
    let runs = create_signal(false);
    let latency = create_signal(START_LATENCY);
    let flight = create_signal((0usize, 0usize));
    let same = create_signal(true);
    let seen = create_signal([None::<Caret>, None]);
    let published = Cell::new([None::<Caret>, None]);
    let timer = store(Cell::new(None::<Timer>));

    let pump = Callback::new(move |force: Option<(Side, Amount)>| {
        let now = now();
        let delay = Duration::from_millis(latency.get_untracked() as u64);
        let held = paused.get_untracked();
        let (external, counts, matching, carets, due) = {
            let mut simulation = simulation
                .write()
                .expect("the collaboration simulation was poisoned");
            simulation.hold(held, now);
            let mut sent = published.get();
            for (side, state) in [(Side::Left, &left), (Side::Right, &right)] {
                let current = caret(state);
                if let Some(current) = current
                    && sent[side.index()] != Some(current)
                {
                    simulation.publish(side, current, now);
                    sent[side.index()] = Some(current);
                }
            }
            published.set(sent);
            match force {
                Some((from, Amount::One)) => simulation.step(from, now),
                Some((from, Amount::All)) => simulation.deliver(from, now, None),
                None if !held => {
                    simulation.deliver(Side::Left, now, Some(delay));
                    simulation.deliver(Side::Right, now, Some(delay));
                }
                None => {}
            }
            (
                [
                    simulation.take_external(Side::Left),
                    simulation.take_external(Side::Right),
                ],
                (
                    simulation.in_flight(Side::Left),
                    simulation.in_flight(Side::Right),
                ),
                simulation.converged(),
                [simulation.seen(Side::Left), simulation.seen(Side::Right)],
                (!held).then(|| simulation.next_due(delay)).flatten(),
            )
        };
        if external[0] {
            left.external_edit();
        }
        if external[1] {
            right.external_edit();
        }
        if flight.get_untracked() != counts {
            flight.set(counts);
        }
        if same.get_untracked() != matching {
            same.set(matching);
        }
        if seen.get_untracked() != carets {
            seen.set(carets);
        }
        if let (Some(due), Some(timer)) = (due, timer.with(Cell::get)) {
            timer.start(due.saturating_duration_since(now));
        }
    });
    let pumping = create_timer(move || {
        pump.call(None);
        None
    });
    timer.with(|timer| timer.set(Some(pumping)));
    create_effect(move || {
        left.content().get();
        right.content().get();
        left.cursors().get();
        right.cursors().get();
        paused.get();
        latency.get();
        pump.call(None);
    });

    let left_remote = create_memo(move || {
        left.content().get();
        remote(&left, seen.get()[Side::Left.index()], RIGHT_COLOR)
    });
    let right_remote = create_memo(move || {
        right.content().get();
        remote(&right, seen.get()[Side::Right.index()], LEFT_COLOR)
    });
    let latency_label = create_memo(move || format!("Latency: {} ms", latency.get().round()));
    let status = create_memo(move || match (flight.get(), same.get()) {
        (_, true) => match rich {
            true => "Both sides hold the same text and styles.".to_owned(),
            false => "Both sides hold the same text.".to_owned(),
        },
        ((0, 0), false) => "The sides differ with nothing in flight.".to_owned(),
        ((right, left), false) => format!("In flight: {right} to the right, {left} to the left."),
    });
    let network = create_memo(move || match paused.get() {
        true => ICON_PLAY,
        false => ICON_PAUSE,
    });
    let narrow = narrower_than(COLUMNS_BREAKPOINT);

    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "The left side orders every edit, as a session's owner does. The right \
                side shows its own edits at once and holds them until the left has ordered \
                them. Pause the network to type on both sides, then send each side's edits \
                when you like. Runs shows where each run of text starts, named by the client \
                that typed it and its offset, in that side's color, with deleted runs struck \
                through."
            </Text>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
                <ToggleButton
                    @test_id="demo.collaboration.pause"
                    label="Network"
                    glyph={network}
                    pressed={paused}
                />
                <ToggleButton @test_id="demo.collaboration.runs" label="Runs" pressed={runs} />
                <Text style=TextStyle::CAPTION>{latency_label}</Text>
            </List>
            <Slider value={latency} max=MAX_LATENCY label="Latency" />
            <unstyled::Stack spacing={use_space(ROW_SPACING)} narrow={narrow}>
                <EditorPane
                    @sizing=ItemSize::Percent(50.0)
                    id="demo.collaboration.left"
                    title="Left, the owner"
                    state={left}
                    remote={left_remote}
                    runs={runs.read_only()}
                    rich
                />
                <SendButtons
                    send={pump}
                    flight={flight.read_only()}
                    paused={paused.read_only()}
                    narrow
                />
                <EditorPane
                    @sizing=ItemSize::Percent(50.0)
                    id="demo.collaboration.right"
                    title="Right, a follower"
                    state={right}
                    remote={right_remote}
                    runs={runs.read_only()}
                    rich
                />
            </unstyled::Stack>
            <Text style=TextStyle::CAPTION>{status}</Text>
        </List>
    }
}

#[derive(Clone, Copy)]
enum Amount {
    One,
    All,
}

type Send = Callback<Option<(Side, Amount)>>;

#[sample]
#[component]
fn SendButtons(
    send: Send,
    flight: ReadSignal<(usize, usize)>,
    paused: ReadSignal<bool>,
    narrow: Memo<bool>,
) -> NodeId {
    let direction = create_memo(move || match narrow.get() {
        true => Direction::Horizontal,
        false => Direction::Vertical,
    });
    let arrows = |wide: [beui::icons::Glyph; 4], stacked: [beui::icons::Glyph; 4]| {
        (0..4)
            .map(|index| {
                create_memo(move || match narrow.get() {
                    true => stacked[index],
                    false => wide[index],
                })
            })
            .collect::<Vec<_>>()
    };
    let glyphs = arrows(
        [
            ICON_CHEVRONS_RIGHT,
            ICON_CHEVRON_RIGHT,
            ICON_CHEVRON_LEFT,
            ICON_CHEVRONS_LEFT,
        ],
        [
            ICON_CHEVRONS_DOWN,
            ICON_CHEVRON_DOWN,
            ICON_CHEVRON_UP,
            ICON_CHEVRONS_UP,
        ],
    );
    let held = |from: Side| {
        create_memo(move || {
            let (right, left) = flight.get();
            let waiting = match from {
                Side::Left => right,
                Side::Right => left,
            };
            !paused.get() || waiting == 0
        })
    };
    let (from_left, from_right) = (held(Side::Left), held(Side::Right));
    let press = |from: Side, amount: Amount| move || send.call(Some((from, amount)));
    view! {
        <List direction align=Align::Center spacing=ROW_SPACING>
            <Button
                icon_only=true
                @test_id="demo.collaboration.all_right"
                glyph={glyphs[0]}
                label="Send every edit from the left"
                disabled={from_left}
                on_click={press(Side::Left, Amount::All)}
            />
            <Button
                icon_only=true
                @test_id="demo.collaboration.one_right"
                glyph={glyphs[1]}
                label="Send one edit from the left"
                disabled={from_left}
                on_click={press(Side::Left, Amount::One)}
            />
            <Button
                icon_only=true
                @test_id="demo.collaboration.one_left"
                glyph={glyphs[2]}
                label="Send one edit from the right"
                disabled={from_right}
                on_click={press(Side::Right, Amount::One)}
            />
            <Button
                icon_only=true
                @test_id="demo.collaboration.all_left"
                glyph={glyphs[3]}
                label="Send every edit from the right"
                disabled={from_right}
                on_click={press(Side::Right, Amount::All)}
            />
        </List>
    }
}

#[sample]
#[component]
fn EditorPane(
    id: &'static str,
    title: &'static str,
    state: TextAreaState,
    remote: Memo<Vec<RemoteTextCursor>>,
    runs: ReadSignal<bool>,
    rich: bool,
) -> NodeId {
    view! {
        <List spacing=ROW_SPACING>
            <Text style=TextStyle::CAPTION>{title}</Text>
            <Show condition={rich}>
                <FormatBar state={state} glyphs={GLYPHS.to_vec()} test_prefix={id} />
            </Show>
            <TextArea
                @test_id=id
                @sizing=ItemSize::Fixed(EDITOR_HEIGHT)
                state={state}
                remote_cursors={remote}
                show_runs={runs}
                client_colors={vec![
                    (Side::Left.client(), LEFT_COLOR),
                    (Side::Right.client(), RIGHT_COLOR),
                ]}
                marker={render_marker()}
            />
        </List>
    }
}
