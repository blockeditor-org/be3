use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use beui::reactive::{
    Align, List, Memo, ReadSignal, Show, Timer, clone, create_effect, create_memo, create_signal,
    create_timer, now, untrack, view,
};
use beui::styled::{Button, ButtonVariant, Caption, Slider, Stack, TextArea};
use beui::unstyled::{RemoteTextCursor, TextAreaState};
use beui::{Color32, Direction, ItemSize, NodeId};
use beui_macros::{component, sample};
use text_editor_core::{CursorPosition, EditorCommand, TextLanguage};

use super::{Caret, Side, SideDocument, Simulation};
use crate::sample::{Sample, ScrollPage};
use crate::styled_pages::LabelledSwitch;
use crate::{ROW_SPACING, SECTION_SPACING};

const TEXT: &str = "# Shared notes\n\nType on both sides. Each side keeps its own copy and \
sends its edits over a simulated network.\n\n- [ ] type at the same place on both sides\n";
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
                <ConcurrentEditing />
            </Sample>
        </ScrollPage>
    }
}

fn editor(simulation: &Arc<RwLock<Simulation>>, side: Side) -> TextAreaState {
    let document = Arc::new(SideDocument::new(simulation, side));
    let state = TextAreaState::new(document as Arc<dyn text_editor_core::Document>);
    state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
    state
}

fn caret(state: &TextAreaState) -> Option<Caret> {
    state.core().cursor_positions().first().map(|cursor| Caret {
        anchor: cursor.pos.anchor,
        focus: cursor.pos.focus,
    })
}

fn remote(state: &TextAreaState, caret: Option<Caret>, color: Color32) -> Vec<RemoteTextCursor> {
    let core = state.core();
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
}

#[sample]
#[component]
fn ConcurrentEditing() -> NodeId {
    let simulation = Arc::new(RwLock::new(Simulation::new(TEXT)));
    let left = editor(&simulation, Side::Left);
    let right = editor(&simulation, Side::Right);
    let (paused, set_paused) = create_signal(false);
    let (latency, set_latency) = create_signal(START_LATENCY);
    let (flight, set_flight) = create_signal((0usize, 0usize));
    let (same, set_same) = create_signal(true);
    let (seen, set_seen) = create_signal([None::<Caret>, None]);
    let published = Rc::new(Cell::new([None::<Caret>, None]));
    let timer: Rc<RefCell<Option<Timer>>> = Rc::new(RefCell::new(None));

    let pump = Rc::new(
        clone!(simulation left right paused latency flight set_flight same set_same seen set_seen published timer -> move |force: Option<Side>| {
            let now = now();
            let delay = Duration::from_millis(latency.get_untracked() as u64);
            let held = paused.get_untracked();
            let (external, counts, matching, carets, due) = {
                let mut simulation = simulation
                    .write()
                    .expect("the collaboration simulation was poisoned");
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
                    Some(from) => simulation.deliver(from, now, None),
                    None if !held => {
                        simulation.deliver(Side::Left, now, Some(delay));
                        simulation.deliver(Side::Right, now, Some(delay));
                    }
                    None => {}
                }
                (
                    [simulation.take_external(Side::Left), simulation.take_external(Side::Right)],
                    (simulation.in_flight(Side::Left), simulation.in_flight(Side::Right)),
                    simulation.text(Side::Left) == simulation.text(Side::Right),
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
                set_flight.set(counts);
            }
            if same.get_untracked() != matching {
                set_same.set(matching);
            }
            if seen.get_untracked() != carets {
                set_seen.set(carets);
            }
            if let (Some(due), Some(timer)) = (due, timer.borrow().as_ref()) {
                timer.start(due.saturating_duration_since(now));
            }
        }),
    );
    *timer.borrow_mut() = Some(create_timer(clone!(pump -> move || {
        pump(None);
        None
    })));
    create_effect(clone!(left right paused latency pump -> move || {
        left.content().get();
        right.content().get();
        left.cursors().get();
        right.cursors().get();
        paused.get();
        latency.get();
        untrack(|| pump(None));
    }));

    let left_remote = create_memo(clone!(left seen -> move || {
        left.content().get();
        remote(&left, seen.get()[Side::Left.index()], RIGHT_COLOR)
    }));
    let right_remote = create_memo(clone!(right seen -> move || {
        right.content().get();
        remote(&right, seen.get()[Side::Right.index()], LEFT_COLOR)
    }));
    let latency_label =
        create_memo(clone!(latency -> move || format!("Latency: {} ms", latency.get().round())));
    let status = create_memo(clone!(flight -> move || match (flight.get(), same.get()) {
        (_, true) => "Both sides hold the same text.".to_owned(),
        ((0, 0), false) => "The sides differ with nothing in flight.".to_owned(),
        ((right, left), false) => format!(
            "In flight: {right} to the right, {left} to the left."
        ),
    }));
    let send: Send = pump.clone();
    let switched = paused.clone();

    view! {
        <List spacing=SECTION_SPACING>
            <Caption
                content="The left side orders every edit, as a session's owner does. The right \
                side shows its own edits at once and holds them until the left has ordered \
                them. Pause the network to type on both sides, then send each side's edits \
                when you like."
                wrap=true
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
                <LabelledSwitch
                    @test_id="demo.collaboration.pause"
                    label="Pause the network"
                    on={switched.clone()}
                    on_change={move |on: bool| set_paused.set(on)}
                />
                <Caption content={latency_label} />
            </List>
            <Slider
                value={latency}
                max=MAX_LATENCY
                label="Latency"
                on_change={move |value| set_latency.set(value)}
            />
            <Show condition={paused}>
                <SendButtons send={send.clone()} flight={flight.clone()} />
            </Show>
            <Stack spacing=ROW_SPACING breakpoint=COLUMNS_BREAKPOINT>
                <EditorPane
                    @sizing=ItemSize::Percent(50.0)
                    id="demo.collaboration.left"
                    title="Left, the owner"
                    state={left}
                    remote={left_remote}
                />
                <EditorPane
                    @sizing=ItemSize::Percent(50.0)
                    id="demo.collaboration.right"
                    title="Right, a follower"
                    state={right}
                    remote={right_remote}
                />
            </Stack>
            <Caption content={status} wrap=true />
        </List>
    }
}

type Send = Rc<dyn Fn(Option<Side>)>;

#[sample]
#[component]
fn SendButtons(send: Send, flight: ReadSignal<(usize, usize)>) -> NodeId {
    let to_right =
        create_memo(clone!(flight -> move || format!("Send left to right ({})", flight.get().0)));
    let to_left = create_memo(move || format!("Send right to left ({})", flight.get().1));
    let send_right = clone!(send -> move || send(Some(Side::Left)));
    let send_left = move || send(Some(Side::Right));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
            <Button
                @test_id="demo.collaboration.send_right"
                label={to_right}
                variant=ButtonVariant::Secondary
                on_click={send_right}
            />
            <Button
                @test_id="demo.collaboration.send_left"
                label={to_left}
                variant=ButtonVariant::Secondary
                on_click={send_left}
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
) -> NodeId {
    view! {
        <List spacing=ROW_SPACING>
            <Caption content={title} />
            <TextArea
                @test_id=id
                @sizing=ItemSize::Fixed(EDITOR_HEIGHT)
                state={state}
                remote_cursors={remote}
            />
        </List>
    }
}
