use std::cell::Cell;

use beui::reactive::{
    Align, ClickCatcher, Direction, Draw, Drawing, ForEach, Frame, ItemSize, List, Memo, Prop,
    Show, Spacer, clone, component, component_rect, create_memo, create_signal, view,
};
use beui::styled::{
    Accordion, Body, Button, ButtonVariant, Caption, Code, Dialog, ListRow, Scroll, Separator,
    use_theme,
};
use beui::{FontId, NodeId, Painter, PointerPress};

use super::session::Session;
use super::*;

const SPACING: f32 = 8.0;
const LABEL_WIDTH: f32 = 110.0;
const CELL_WIDTH: f32 = 52.0;
const DIALOG_WIDTH: f32 = 260.0;
const GRAPH_EDGE_COLOR: Color32 = Color32::from_rgb(140, 150, 170);
const GRAPH_OUTLINE_COLOR: Color32 = Color32::from_rgb(70, 78, 96);
const GRAPH_DETAIL_COLOR: Color32 = Color32::from_rgb(225, 225, 225);

#[component]
pub(super) fn Panels(session: Rc<Session>) -> NodeId {
    let (challenge_open, set_challenge_open) = create_signal(true);
    let (simulation_open, set_simulation_open) = create_signal(true);
    let (metrics_open, set_metrics_open) = create_signal(true);
    let (debugger_open, set_debugger_open) = create_signal(false);
    let (graph_open, set_graph_open) = create_signal(false);
    let challenged = create_memo(clone!(session -> move || {
        session.read(|model| model.challenge.is_some())
    }));
    let challenge_session = Rc::clone(&session);
    let simulation_session = Rc::clone(&session);
    let metrics_session = Rc::clone(&session);
    let debugger_session = Rc::clone(&session);
    let graph_session = Rc::clone(&session);
    view! {
        <List spacing=SPACING>
            <Show condition={challenged}>
                <Accordion
                    title="Challenge"
                    open={challenge_open}
                    on_toggle={move |open| set_challenge_open.set(open)}
                >
                    <ChallengePanel session={challenge_session} />
                </Accordion>
            </Show>
            <Accordion
                title="Simulation"
                open={simulation_open}
                on_toggle={move |open| set_simulation_open.set(open)}
            >
                <SimulationPanel session={simulation_session} />
            </Accordion>
            <Accordion
                title="Metrics"
                open={metrics_open}
                on_toggle={move |open| set_metrics_open.set(open)}
            >
                <MetricsPanel session={metrics_session} />
            </Accordion>
            <Accordion
                title="Grid debugger"
                open={debugger_open}
                on_toggle={move |open| set_debugger_open.set(open)}
            >
                <DebuggerPanel session={debugger_session} />
            </Accordion>
            <Accordion
                title="Generated graph"
                open={graph_open}
                on_toggle={move |open| set_graph_open.set(open)}
            >
                <GraphPanel session={graph_session} />
            </Accordion>
            <StorageDialog session={session} />
        </List>
    }
}

#[component]
fn Field(label: Prop<String>, value: Prop<String>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Caption @sizing=ItemSize::Fixed(LABEL_WIDTH) content={label} />
            <Code @sizing=ItemSize::Percent(100.0) content={value} />
        </List>
    }
}

#[component]
fn Problem(content: Prop<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <Caption content={content} color={theme.danger.clone()} wrap=true />
    }
}

#[component]
fn Rows(rows: Memo<Vec<(String, String)>>) -> NodeId {
    let keys = create_memo(clone!(rows -> move || (0..rows.with(Vec::len)).collect::<Vec<_>>()));
    let empty = create_memo(clone!(keys -> move || keys.with(Vec::is_empty)));
    view! {
        <List spacing=4.0>
            <Show condition={empty}>
                <Caption content="None" />
            </Show>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let label = create_memo(clone!(rows -> move || {
                        rows.with(|rows| rows.get(index).map(|row| row.0.clone()).unwrap_or_default())
                    }));
                    let value = create_memo(clone!(rows -> move || {
                        rows.with(|rows| rows.get(index).map(|row| row.1.clone()).unwrap_or_default())
                    }));
                    view! {
                        <Field label={label} value={value} />
                    }
                }}
            </ForEach>
        </List>
    }
}

fn or_none(value: Option<String>) -> String {
    value.unwrap_or_else(|| "none".to_owned())
}

#[component]
fn MetricsPanel(session: Rc<Session>) -> NodeId {
    let error = create_memo(clone!(session -> move || {
        session.read(|model| model.simulation.error.clone())
    }));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let problem = create_memo(clone!(error -> move || {
        format!("Cannot compile: {}", error.get().unwrap_or_default())
    }));
    let rows = create_memo(clone!(session -> move || {
        session.read(|model| {
            let vm = model.simulation.vm.as_ref();
            vec![
                (
                    "Total instructions".to_owned(),
                    or_none(vm.map(|vm| vm.root_component.total_instruction_count().to_string())),
                ),
                (
                    "Total latency".to_owned(),
                    or_none(vm.map(|vm| vm.root_component.total_latency().to_string())),
                ),
                (
                    "Total area".to_owned(),
                    or_none(model.grid.bounds().map(|bounds| {
                        format!(
                            "{} ({} x {})",
                            bounds.area(),
                            bounds.width(),
                            bounds.height()
                        )
                    })),
                ),
            ]
        })
    }));
    view! {
        <List spacing=SPACING>
            <Show condition={failed}>
                <Problem content={problem} />
            </Show>
            <Rows rows={rows} />
        </List>
    }
}

type InputView = (String, Option<Vec<(u32, u64)>>);

type TickView = (Vec<(String, Option<bool>)>, bool);

#[derive(Clone, Debug, PartialEq)]
struct SimulationView {
    error: Option<String>,
    compiled: bool,
    summary: Vec<(String, String)>,
    frames: Vec<(String, bool, SimulationFrame)>,
    instructions: Vec<(String, bool, Option<bool>)>,
    inputs: Vec<InputView>,
    outputs: Vec<(String, String)>,
    wires: Vec<(String, String)>,
    storage: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimulationFrame {
    Root,
    Caller(usize),
    Current,
}

fn value_text(value: u64) -> String {
    format!("0x{value:016x} ({value})")
}

impl LogicGridEditor {
    fn simulation_view(&self) -> SimulationView {
        let mut shown = SimulationView {
            error: self.simulation.error.clone(),
            compiled: false,
            summary: Vec::new(),
            frames: Vec::new(),
            instructions: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            wires: Vec::new(),
            storage: Vec::new(),
        };
        let (Some(vm), Some(snapshot)) = (&self.simulation.vm, &self.simulation.snapshot) else {
            return shown;
        };
        shown.compiled = true;
        let selection = &self.simulation.instruction_selection;
        let view = simulation_instruction_view(vm, selection, self.simulation.tick_in_progress);
        let next_here = match view.next_instruction {
            Some(next) => format!("{} / {}", next + 1, view.instructions.len()),
            None if view.instructions.is_empty() => "none".to_owned(),
            None => "not active".to_owned(),
        };
        shown.summary = vec![
            ("Steps".to_owned(), self.simulation.steps.to_string()),
            (
                "Instructions".to_owned(),
                view.instructions.len().to_string(),
            ),
            ("Viewing".to_owned(), view.name.clone()),
            ("Next here".to_owned(), next_here),
        ];

        let root_active = matches!(selection, SimulationInstructionSelection::ReturnFrame(0))
            || vm.returns.is_empty() && matches!(selection, SimulationInstructionSelection::Active);
        shown
            .frames
            .push(("Root".to_owned(), root_active, SimulationFrame::Root));
        for (index, pc) in vm.returns.iter().enumerate().skip(1) {
            shown.frames.push((
                format!(
                    "Caller {index}: {}",
                    simulation_component_name(&pc.component)
                ),
                matches!(selection, SimulationInstructionSelection::ReturnFrame(selected) if *selected == index),
                SimulationFrame::Caller(index),
            ));
        }
        if !vm.returns.is_empty() {
            shown.frames.push((
                format!("Current: {}", simulation_component_name(&vm.pc.component)),
                matches!(selection, SimulationInstructionSelection::Active),
                SimulationFrame::Current,
            ));
        }

        for (index, instruction) in view.instructions.iter().enumerate() {
            let target = match instruction {
                Instruction::Call { component, .. } => {
                    Some(view.component.components.get(*component).is_some())
                }
                _ => None,
            };
            shown.instructions.push((
                format!("{index:03}  {instruction}"),
                view.next_instruction == Some(index),
                target,
            ));
        }

        let mut input_ports = snapshot
            .components
            .iter()
            .filter_map(|component| match component.kind {
                ComponentKind::Input { scale, id, .. } => Some((id, scale)),
                _ => None,
            })
            .collect::<Vec<_>>();
        input_ports.sort_by_key(|(id, _)| *id);
        for (input, address) in vm.input_addresses().iter().copied().enumerate() {
            let label = format!("Input {input}");
            let value = self.simulation.input_values.get(input).copied();
            let scale = input_ports.get(input).map(|(_, scale)| *scale);
            let bits = match (value, scale) {
                (Some(value), Some(scale)) if address < vm.root_component.memory_size => Some(
                    storage_bit_indices(scale)
                        .into_iter()
                        .map(|bit| (bit, (value >> bit) & 1))
                        .collect(),
                ),
                _ => None,
            };
            shown.inputs.push((label, bits));
        }

        let output_count = snapshot
            .components
            .iter()
            .filter(|component| matches!(component.kind, ComponentKind::Output { .. }))
            .count();
        for (output, &address) in vm.output_addresses().iter().enumerate() {
            let value = (output < output_count)
                .then(|| vm.root_memory().get(address).copied())
                .flatten();
            shown.outputs.push((
                format!("Output {output}"),
                value.map_or_else(|| "deleted".to_owned(), value_text),
            ));
        }

        for (address, value) in vm.root_memory().iter().copied().enumerate() {
            let segments = snapshot
                .graph
                .nodes
                .iter()
                .filter_map(|node| match node {
                    GraphNode::WireNet { wires } => Some(wires.len()),
                    _ => None,
                })
                .nth(address)
                .unwrap_or_default();
            shown.wires.push((
                format!("Memory {address} ({segments} segments)"),
                value_text(value),
            ));
        }

        for (storage, value) in vm.storage.iter().copied().enumerate() {
            let component = snapshot
                .components
                .iter()
                .filter(|component| matches!(component.kind, ComponentKind::Storage { .. }))
                .nth(storage)
                .map(|component| component.id);
            let label = component.map_or_else(
                || format!("Storage {storage}"),
                |component| format!("Storage #{}", component.0),
            );
            shown.storage.push((label, value_text(value)));
        }
        shown
    }

    fn select_simulation_frame(&mut self, frame: SimulationFrame) {
        let Some(vm) = &self.simulation.vm else {
            return;
        };
        self.simulation.instruction_selection = match frame {
            SimulationFrame::Root if vm.returns.is_empty() => {
                SimulationInstructionSelection::Active
            }
            SimulationFrame::Root => SimulationInstructionSelection::ReturnFrame(0),
            SimulationFrame::Caller(index) => SimulationInstructionSelection::ReturnFrame(index),
            SimulationFrame::Current => SimulationInstructionSelection::Active,
        };
    }

    fn select_instruction_target(&mut self, index: usize) {
        let Some(vm) = &self.simulation.vm else {
            return;
        };
        let view = simulation_instruction_view(
            vm,
            &self.simulation.instruction_selection,
            self.simulation.tick_in_progress,
        );
        let Some(Instruction::Call { component, .. }) = view.instructions.get(index) else {
            return;
        };
        let Some(target) = view.component.components.get(*component).map(Rc::clone) else {
            return;
        };
        self.simulation.instruction_selection = SimulationInstructionSelection::Component(target);
    }
}

#[component]
fn SimulationPanel(session: Rc<Session>) -> NodeId {
    let shown =
        create_memo(clone!(session -> move || session.read(LogicGridEditor::simulation_view)));
    let failed = create_memo(clone!(shown -> move || shown.get().error.is_some()));
    let problem = create_memo(clone!(shown -> move || {
        format!("Cannot run: {}", shown.get().error.unwrap_or_default())
    }));
    let idle = create_memo(clone!(shown -> move || {
        let shown = shown.get();
        shown.error.is_none() && !shown.compiled
    }));
    let compiled = create_memo(clone!(shown -> move || shown.get().compiled));
    let summary = create_memo(clone!(shown -> move || shown.get().summary));
    let frames =
        create_memo(clone!(shown -> move || (0..shown.get().frames.len()).collect::<Vec<_>>()));
    let instructions = create_memo(clone!(shown -> move || {
        (0..shown.get().instructions.len()).collect::<Vec<_>>()
    }));
    let no_instructions =
        create_memo(clone!(instructions -> move || instructions.get().is_empty()));
    let inputs =
        create_memo(clone!(shown -> move || (0..shown.get().inputs.len()).collect::<Vec<_>>()));
    let no_inputs = create_memo(clone!(inputs -> move || inputs.with(Vec::is_empty)));
    let outputs = create_memo(clone!(shown -> move || shown.get().outputs));
    let wires = create_memo(clone!(shown -> move || shown.get().wires));
    let storage = create_memo(clone!(shown -> move || shown.get().storage));
    let tick = clone!(session -> move || session.update(LogicGridEditor::run_simulation_tick));
    let step =
        clone!(session -> move || session.update(LogicGridEditor::run_simulation_instruction));
    let frame_session = Rc::clone(&session);
    let frame_shown = shown.clone();
    let instruction_session = Rc::clone(&session);
    let instruction_shown = shown.clone();
    let input_shown = shown.clone();
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Button
                    label="Run tick"
                    variant=ButtonVariant::Secondary
                    @test_id={"logic-grid.run-tick"}
                    on_click={tick}
                />
                <Button
                    label="Step instruction"
                    variant=ButtonVariant::Secondary
                    @test_id={"logic-grid.step-instruction"}
                    on_click={step}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <Show condition={failed}>
                <Problem content={problem} />
            </Show>
            <Show condition={idle}>
                <Caption content="Run a step to compile and execute the circuit." wrap=true />
            </Show>
            <Show condition={compiled}>
                <List spacing=SPACING>
                    <Rows rows={summary} />
                    <Separator />
                    <Body content="Call stack" />
                    <List spacing=2.0>
                        <ForEach keys={frames}>
                            {move |index: usize| {
                                let session = Rc::clone(&frame_session);
                                let row = create_memo(clone!(frame_shown -> move || {
                                    frame_shown.get().frames.get(index).cloned()
                                }));
                                let label = create_memo(clone!(row -> move || {
                                    row.get().map(|row| row.0).unwrap_or_default()
                                }));
                                let selected = create_memo(clone!(row -> move || {
                                    row.get().is_some_and(|row| row.1)
                                }));
                                let chose = move || {
                                    if let Some((_, _, frame)) = row.get_untracked() {
                                        session.update(|model| model.select_simulation_frame(frame));
                                    }
                                };
                                view! {
                                    <ListRow selected={selected} on_click={chose}>
                                        <Body content={label} />
                                    </ListRow>
                                }
                            }}
                        </ForEach>
                    </List>
                    <Separator />
                    <Body content="Instructions" />
                    <Show condition={no_instructions}>
                        <Caption content="No instructions" />
                    </Show>
                    <List spacing=2.0>
                        <ForEach keys={instructions}>
                            {move |index: usize| {
                                let session = Rc::clone(&instruction_session);
                                let row = create_memo(clone!(instruction_shown -> move || {
                                    instruction_shown.get().instructions.get(index).cloned()
                                }));
                                view! {
                                    <InstructionRow session={session} row={row} index={index} />
                                }
                            }}
                        </ForEach>
                    </List>
                    <Separator />
                    <Body content="Inputs" />
                    <Show condition={no_inputs}>
                        <Caption content="None" />
                    </Show>
                    <List spacing=4.0>
                        <ForEach keys={inputs}>
                            {move |index: usize| {
                                let session = Rc::clone(&session);
                                let row = create_memo(clone!(input_shown -> move || {
                                    input_shown.get().inputs.get(index).cloned()
                                }));
                                view! {
                                    <InputRow session={session} row={row} index={index} />
                                }
                            }}
                        </ForEach>
                    </List>
                    <Separator />
                    <Body content="Outputs" />
                    <Rows rows={outputs} />
                    <Separator />
                    <Body content="Wire groups" />
                    <Rows rows={wires} />
                    <Separator />
                    <Body content="Storage" />
                    <Rows rows={storage} />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn InstructionRow(
    session: Rc<Session>,
    row: Memo<Option<(String, bool, Option<bool>)>>,
    index: usize,
) -> NodeId {
    let text = create_memo(clone!(row -> move || row.get().map(|row| row.0).unwrap_or_default()));
    let theme = use_theme();
    let fill = create_memo(
        clone!(row theme -> move || match row.get().is_some_and(|row| row.1) {
            true => theme.accent_soft.get(),
            false => Color32::TRANSPARENT,
        }),
    );
    let calls = create_memo(clone!(row -> move || row.get().is_some_and(|row| row.2.is_some())));
    let unresolved =
        create_memo(clone!(row -> move || row.get().is_none_or(|row| row.2 != Some(true))));
    let target = move || session.update(|model| model.select_instruction_target(index));
    view! {
        <Frame color={fill} radius=3 padding_horizontal=4.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Code @sizing=ItemSize::Percent(100.0) content={text} />
                <Show condition={calls}>
                    <Button
                        label="Target"
                        variant=ButtonVariant::Secondary
                        disabled={unresolved}
                        on_click={target}
                    />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn InputRow(session: Rc<Session>, row: Memo<Option<InputView>>, index: usize) -> NodeId {
    let label = create_memo(clone!(row -> move || row.get().map(|row| row.0).unwrap_or_default()));
    let deleted = create_memo(clone!(row -> move || row.get().is_none_or(|row| row.1.is_none())));
    let bits = create_memo(clone!(row -> move || {
        row.get()
            .and_then(|row| row.1)
            .map(|bits| bits.into_iter().map(|(bit, _)| bit).collect::<Vec<_>>())
            .unwrap_or_default()
    }));
    view! {
        <List direction=Direction::Horizontal wrap=true align=Align::Center spacing=4.0>
            <Caption content={label} />
            <Show condition={deleted}>
                <Caption content="deleted" />
            </Show>
            <ForEach keys={bits}>
                {move |bit: u32| {
                    let session = Rc::clone(&session);
                    let state = create_memo(clone!(row -> move || {
                        row.get()
                            .and_then(|row| row.1)
                            .and_then(|bits| bits.into_iter().find(|(held, _)| *held == bit))
                            .map_or(0, |(_, state)| state)
                    }));
                    let text = create_memo(move || format!("{bit}:{}", state.get()));
                    let toggle = move || {
                        session.update(|model| model.toggle_simulation_input_bit(index, bit));
                    };
                    view! {
                        <Button label={text} variant=ButtonVariant::Secondary on_click={toggle} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ChallengeView {
    goal: String,
    status: String,
    error: Option<String>,
    header: Vec<String>,
    rows: Vec<TickView>,
}

impl LogicGridEditor {
    fn challenge_view(&self) -> Option<ChallengeView> {
        let challenge = self.challenge.as_ref()?;
        let data = &challenge.data;
        let test = &challenge.test;
        let all_ports = test.input_slots.iter().all(Option::is_some)
            && test.output_slots.iter().all(Option::is_some);
        let status = if !all_ports {
            "Place every challenge port to run the test".to_owned()
        } else if test.mismatched {
            "Failed".to_owned()
        } else if test.next_tick == 0 {
            "Not run".to_owned()
        } else if test.next_tick < data.ticks {
            format!("Running {}/{}", test.next_tick, data.ticks)
        } else {
            "Passed".to_owned()
        };
        let mut header = vec!["Tick".to_owned()];
        header.extend(data.inputs.iter().map(|port| port.label.to_owned()));
        header.extend(data.outputs.iter().map(|port| port.label.to_owned()));
        let active = test.next_tick.checked_sub(1);
        let rows = (0..data.ticks)
            .map(|tick| {
                let mut cells = vec![(tick.to_string(), Some(true))];
                for port in &data.inputs {
                    let value = port.values.get(tick).copied().unwrap_or(0);
                    cells.push((value.to_string(), Some(true)));
                }
                for (index, port) in data.outputs.iter().enumerate() {
                    let expected = port.values.get(tick).copied().unwrap_or(0);
                    if tick < test.next_tick {
                        let actual = test
                            .actual
                            .get(index)
                            .and_then(|values| values.get(tick))
                            .copied()
                            .unwrap_or(expected);
                        cells.push((actual.to_string(), Some(actual == expected)));
                    } else {
                        cells.push((expected.to_string(), None));
                    }
                }
                (cells, active == Some(tick))
            })
            .collect();
        Some(ChallengeView {
            goal: data.goal.to_owned(),
            status,
            error: test.error.clone(),
            header,
            rows,
        })
    }
}

#[component]
fn ChallengePanel(session: Rc<Session>) -> NodeId {
    let shown =
        create_memo(clone!(session -> move || session.read(LogicGridEditor::challenge_view)));
    let goal = create_memo(
        clone!(shown -> move || shown.get().map(|shown| shown.goal).unwrap_or_default()),
    );
    let status = create_memo(
        clone!(shown -> move || shown.get().map(|shown| shown.status).unwrap_or_default()),
    );
    let error = create_memo(clone!(shown -> move || shown.get().and_then(|shown| shown.error)));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let working = create_memo(clone!(error -> move || error.get().is_none()));
    let problem = create_memo(
        clone!(error -> move || format!("Cannot run: {}", error.get().unwrap_or_default())),
    );
    let header = create_memo(clone!(shown -> move || {
        shown.get().map(|shown| shown.header).unwrap_or_default()
    }));
    let ticks = create_memo(clone!(shown -> move || {
        (0..shown.get().map_or(0, |shown| shown.rows.len())).collect::<Vec<_>>()
    }));
    let step = clone!(session -> move || session.update(LogicGridEditor::challenge_test_step));
    let run = clone!(session -> move || session.update(LogicGridEditor::challenge_test_run_all));
    let reset = clone!(session -> move || session.update(LogicGridEditor::challenge_test_reset));
    let rows_shown = shown.clone();
    view! {
        <List spacing=SPACING>
            <Body content={goal} />
            <List direction=Direction::Horizontal wrap=true align=Align::Center spacing=SPACING>
                <Button
                    label="Step test"
                    variant=ButtonVariant::Secondary
                    @test_id={"logic-grid.challenge-step"}
                    on_click={step}
                />
                <Button
                    label="Run all tests"
                    variant=ButtonVariant::Primary
                    @test_id={"logic-grid.challenge-run"}
                    on_click={run}
                />
                <Button label="Reset" variant=ButtonVariant::Secondary on_click={reset} />
            </List>
            <Show condition={failed}>
                <Problem content={problem} />
            </Show>
            <Show condition={working}>
                <List spacing=2.0>
                    <Caption content={status} @test_id={"logic-grid.challenge-status"} />
                    <Scroll direction=Direction::Horizontal>
                        <List spacing=2.0>
                            <TableRow cells={header.clone()} />
                            <ForEach keys={ticks}>
                                {move |tick: usize| {
                                    let session = Rc::clone(&session);
                                    let row = create_memo(clone!(rows_shown -> move || {
                                        rows_shown
                                            .get()
                                            .and_then(|shown| shown.rows.get(tick).cloned())
                                    }));
                                    view! {
                                        <TickRow session={session} row={row} tick={tick} />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Scroll>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn TableRow(cells: Memo<Vec<String>>) -> NodeId {
    let keys = create_memo(clone!(cells -> move || (0..cells.with(Vec::len)).collect::<Vec<_>>()));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let text = create_memo(clone!(cells -> move || {
                        cells.with(|cells| cells.get(index).cloned().unwrap_or_default())
                    }));
                    view! {
                        <Frame @sizing=ItemSize::Fixed(CELL_WIDTH) padding_horizontal=2.0>
                            <Body content={text} />
                        </Frame>
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TickRow(session: Rc<Session>, row: Memo<Option<TickView>>, tick: usize) -> NodeId {
    let active = create_memo(clone!(row -> move || row.get().is_some_and(|row| row.1)));
    let keys = create_memo(clone!(row -> move || {
        (0..row.get().map_or(0, |row| row.0.len())).collect::<Vec<_>>()
    }));
    let seek = move || session.update(|model| model.challenge_test_seek(tick));
    view! {
        <ListRow selected={active} on_click={seek}>
            <List direction=Direction::Horizontal spacing=0.0>
                <ForEach keys={keys}>
                    {move |index: usize| {
                        let cell = create_memo(clone!(row -> move || {
                            row.get().and_then(|row| row.0.get(index).cloned())
                        }));
                        view! {
                            <TickCell cell={cell} />
                        }
                    }}
                </ForEach>
            </List>
        </ListRow>
    }
}

#[component]
fn TickCell(cell: Memo<Option<(String, Option<bool>)>>) -> NodeId {
    let text =
        create_memo(clone!(cell -> move || cell.get().map(|cell| cell.0).unwrap_or_default()));
    let theme = use_theme();
    let color = create_memo(
        clone!(cell theme -> move || match cell.get().and_then(|cell| cell.1) {
            Some(true) => theme.text.get(),
            Some(false) => theme.danger.get(),
            None => theme.text_muted.get(),
        }),
    );
    view! {
        <Frame width=CELL_WIDTH padding_horizontal=2.0>
            <Code content={text} color={color} />
        </Frame>
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EntityRow {
    title: String,
    details: Vec<(String, String)>,
    entity: DebugEntity,
}

impl LogicGridEditor {
    fn entity_rows(&self) -> Vec<EntityRow> {
        let components = self.grid.components().map(|component| EntityRow {
            title: format!(
                "#{} {}",
                component.id.0,
                component_kind_name(&component.kind)
            ),
            details: vec![
                (
                    "Position".to_owned(),
                    format!("({}, {})", component.position.x, component.position.y),
                ),
                (
                    "Orientation".to_owned(),
                    format!("{:?}", component.orientation),
                ),
                ("Kind".to_owned(), format!("{:?}", component.kind)),
                (
                    "Size".to_owned(),
                    component.size().map_or_else(
                        || "overflow".to_owned(),
                        |size| format!("{} x {}", size.width, size.height),
                    ),
                ),
            ],
            entity: DebugEntity::Component(component.id),
        });
        let wires = self
            .grid
            .wires()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, wire)| EntityRow {
                title: format!("Wire #{index} {:?}", wire.orientation()),
                details: vec![
                    (
                        "Start".to_owned(),
                        format!("({}, {})", wire.start.x, wire.start.y),
                    ),
                    (
                        "End".to_owned(),
                        format!("({}, {})", wire.end.x, wire.end.y),
                    ),
                    ("Length".to_owned(), wire.length().to_string()),
                    ("Scale".to_owned(), format!("{}x", wire.scale.get())),
                ],
                entity: DebugEntity::Wire(wire),
            });
        components.chain(wires).collect()
    }
}

#[component]
fn DebuggerPanel(session: Rc<Session>) -> NodeId {
    let pointer = session.pointer.clone();
    let summary = create_memo(clone!(session pointer -> move || {
        let hovered = pointer.get();
        session.read(|model| {
            let square = hovered.map(|pointer| snap_point(pointer, model.tool.snap()));
            vec![
                (
                    "Components".to_owned(),
                    model.grid.components().count().to_string(),
                ),
                ("Wires".to_owned(), model.grid.wires().len().to_string()),
                (
                    "Bounds".to_owned(),
                    or_none(model.grid.bounds().map(|bounds| {
                        format!(
                            "({}, {}) to ({}, {})",
                            bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y
                        )
                    })),
                ),
                (
                    "Validation errors".to_owned(),
                    model.grid.validate().len().to_string(),
                ),
                (
                    "Hovered square".to_owned(),
                    or_none(square.map(|point| format!("({}, {})", point.x, point.y))),
                ),
            ]
        })
    }));
    let errors = create_memo(clone!(session -> move || {
        session.read(|model| {
            model
                .grid
                .validate()
                .iter()
                .enumerate()
                .map(|(index, error)| format!("#{index} {error:?}"))
                .collect::<Vec<_>>()
        })
    }));
    let valid = create_memo(clone!(errors -> move || errors.with(Vec::is_empty)));
    let error_keys =
        create_memo(clone!(errors -> move || (0..errors.with(Vec::len)).collect::<Vec<_>>()));
    let entities =
        create_memo(clone!(session -> move || session.read(LogicGridEditor::entity_rows)));
    let entity_keys = create_memo(clone!(entities -> move || {
        entities.with(|rows| rows.iter().map(|row| row.entity).collect::<Vec<_>>())
    }));
    let empty = create_memo(clone!(entity_keys -> move || entity_keys.with(Vec::is_empty)));
    view! {
        <List spacing=SPACING>
            <Rows rows={summary} />
            <Separator />
            <Body content="Validation errors" />
            <Show condition={valid}>
                <Caption content="No validation errors" />
            </Show>
            <ForEach keys={error_keys}>
                {move |index: usize| {
                    let text = create_memo(clone!(errors -> move || {
                        errors.with(|errors| errors.get(index).cloned().unwrap_or_default())
                    }));
                    view! {
                        <Problem content={text} />
                    }
                }}
            </ForEach>
            <Separator />
            <Body content="Entities" />
            <Show condition={empty}>
                <Caption content="No entities" />
            </Show>
            <ForEach keys={entity_keys}>
                {move |entity: DebugEntity| {
                    let session = Rc::clone(&session);
                    let row = create_memo(clone!(entities -> move || {
                        entities.with(|rows| rows.iter().find(|row| row.entity == entity).cloned())
                    }));
                    view! {
                        <EntityPanel session={session} row={row} entity={entity} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn EntityPanel(session: Rc<Session>, row: Memo<Option<EntityRow>>, entity: DebugEntity) -> NodeId {
    let (open, set_open) = create_signal(false);
    let title =
        create_memo(clone!(row -> move || row.get().map(|row| row.title).unwrap_or_default()));
    let details =
        create_memo(clone!(row -> move || row.get().map(|row| row.details).unwrap_or_default()));
    let hovered = clone!(session -> move |over: bool| {
        let current = session.debug_hover.get_untracked();
        if over {
            session.set_debug_hover.set(Some(entity));
        } else if current == Some(entity) {
            session.set_debug_hover.set(None);
        }
    });
    view! {
        <ClickCatcher on_hover_change={hovered}>
            <Accordion title={title} open={open} on_toggle={move |open| set_open.set(open)}>
                <Rows rows={details} />
            </Accordion>
        </ClickCatcher>
    }
}

#[derive(Clone, Debug, PartialEq)]
struct GraphShape {
    graph: CircuitGraph,
    positions: Vec<Pos2>,
    size: Vec2,
}

fn paint_graph(painter: &Painter, rect: Rect, shape: &GraphShape) {
    let origin = rect.min;
    let at = |point: Pos2| Pos2::new(origin.x + point.x, origin.y + point.y);
    for edge in &shape.graph.edges {
        painter.line(
            at(shape.positions[edge.first.0]),
            at(shape.positions[edge.second.0]),
            2.0,
            GRAPH_EDGE_COLOR,
        );
    }
    for (index, node) in shape.graph.nodes.iter().enumerate() {
        let center = at(shape.positions[index]);
        let node_rect = Rect::from_min_size(
            Pos2::new(
                center.x - GRAPH_NODE_SIZE.x * 0.5,
                center.y - GRAPH_NODE_SIZE.y * 0.5,
            ),
            GRAPH_NODE_SIZE,
        );
        let (fill, title, detail) = graph_node_display(node);
        painter.rect_filled(node_rect, 6.0, fill);
        painter.rect_stroke(node_rect, 6.0, 1.0, GRAPH_OUTLINE_COLOR);
        for (text, font, color, offset) in [
            (
                format!("#{index} {title}"),
                FontId::proportional(14.0),
                Color32::WHITE,
                -9.0,
            ),
            (detail, FontId::monospace(11.0), GRAPH_DETAIL_COLOR, 10.0),
        ] {
            let galley = painter.layout(text, font, GRAPH_NODE_SIZE.x);
            let size = galley.size();
            painter.galley(
                Pos2::new(center.x - size.x * 0.5, center.y + offset - size.y * 0.5),
                galley,
                color,
            );
        }
    }
}

#[component]
fn GraphPanel(session: Rc<Session>) -> NodeId {
    let shape = create_memo(clone!(session -> move || {
        session.read(|model| {
            let graph = model.grid.generate_graph();
            let (positions, size) = graph_layout(&graph);
            Rc::new(GraphShape {
                graph,
                positions,
                size,
            })
        })
    }));
    let counts = create_memo(clone!(shape -> move || {
        let shape = shape.get();
        vec![
            ("Nodes".to_owned(), shape.graph.nodes.len().to_string()),
            ("Edges".to_owned(), shape.graph.edges.len().to_string()),
        ]
    }));
    let empty = create_memo(clone!(shape -> move || shape.get().graph.nodes.is_empty()));
    let drawn = create_memo(clone!(empty -> move || !empty.get()));
    let width = create_memo(clone!(shape -> move || Some(shape.get().size.x)));
    let height = create_memo(clone!(shape -> move || Some(shape.get().size.y)));
    let painted = shape.clone();
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(move || {
        let shape = painted.get();
        Rc::new(move |painter: &Painter, rect: Rect| paint_graph(painter, rect, &shape)) as Draw
    }));
    view! {
        <List spacing=SPACING>
            <Rows rows={counts} />
            <Show condition={empty}>
                <Caption content="The generated graph is empty." />
            </Show>
            <Show condition={drawn}>
                <Scroll direction=Direction::Horizontal>
                    <GraphCanvas
                        session={session}
                        shape={shape}
                        width={width}
                        height={height}
                        draw={draw}
                    />
                </Scroll>
            </Show>
        </List>
    }
}

#[component]
fn GraphCanvas(
    session: Rc<Session>,
    shape: Memo<Rc<GraphShape>>,
    width: Memo<Option<f32>>,
    height: Memo<Option<f32>>,
    draw: Prop<Draw>,
) -> NodeId {
    let placed = component_rect();
    let hovered = Rc::new(Cell::new(None::<usize>));
    let over = clone!(session hovered -> move |press: PointerPress| {
        let rect = placed.get_untracked();
        let local = Pos2::new(press.pos.x - rect.min.x, press.pos.y - rect.min.y);
        let shape = shape.get_untracked();
        let node = shape.positions.iter().position(|center| {
            (local.x - center.x).abs() <= GRAPH_NODE_SIZE.x * 0.5
                && (local.y - center.y).abs() <= GRAPH_NODE_SIZE.y * 0.5
        });
        if hovered.replace(node) == node {
            return;
        }
        let mut hover = GraphHover::default();
        if let Some(node) = node.and_then(|node| shape.graph.nodes.get(node)) {
            hover.include_node(node);
        }
        session.set_graph_hover.set(hover);
    });
    let left = clone!(session hovered -> move |inside: bool| {
        if !inside {
            hovered.set(None);
            session.set_graph_hover.set(GraphHover::default());
        }
    });
    view! {
        <ClickCatcher on_hover_move={over} on_hover_change={left}>
            <Frame width={width} height={height}>
                <Drawing draw={draw} />
            </Frame>
        </ClickCatcher>
    }
}

#[component]
fn StorageDialog(session: Rc<Session>) -> NodeId {
    let configured = create_memo(clone!(session -> move || {
        session.read(|model| {
            let id = model.configured_storage?;
            match &model.grid.component(id)?.kind {
                ComponentKind::Storage { scale, value } => Some((id, *scale, *value)),
                _ => None,
            }
        })
    }));
    let open = create_memo(clone!(configured -> move || configured.get().is_some()));
    let title = create_memo(clone!(configured -> move || {
        configured
            .get()
            .map(|(id, _, _)| format!("Configure storage #{}", id.0))
            .unwrap_or_default()
    }));
    let bits = create_memo(clone!(configured -> move || {
        configured
            .get()
            .map(|(_, scale, _)| storage_bit_indices(scale))
            .unwrap_or_default()
    }));
    let dismiss =
        clone!(session -> move || session.update(|model| model.configured_storage = None));
    view! {
        <Dialog open={open} title={title} width=DIALOG_WIDTH on_dismiss={dismiss}>
            <List direction=Direction::Horizontal wrap=true spacing=4.0>
                <ForEach keys={bits}>
                    {move |bit: u32| {
                        let session = Rc::clone(&session);
                        let label = create_memo(clone!(configured -> move || {
                            let state = configured.get().map_or(0, |(_, _, value)| (value >> bit) & 1);
                            format!("Bit {bit}: {state}")
                        }));
                        let toggle = clone!(configured -> move || {
                            if let Some((id, _, _)) = configured.get_untracked() {
                                session.edit(|model| model.toggle_storage_bit(id, bit));
                            }
                        });
                        view! {
                            <Button
                                label={label}
                                variant=ButtonVariant::Secondary
                                @test_id={format!("logic-grid.storage-bit.{bit}")}
                                on_click={toggle}
                            />
                        }
                    }}
                </ForEach>
            </List>
        </Dialog>
    }
}
