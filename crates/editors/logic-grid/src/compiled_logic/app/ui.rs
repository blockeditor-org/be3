use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockLink, ChildTarget, Editor};
use logic_grid_types::CompiledLogic as Program;
use logic_grid_types::CompiledLogicContent;
use logic_grid_types::LogicGridContent;
use logicgame::grid::{ComponentPort, ConnectionDirection};
use uuid::Uuid;

const PADDING: Space = Space::Xl2;
const SECTION_SPACING: Space = Space::Xl;
const INTRINSIC_WIDTH: f32 = 640.0;
const ROW_HEIGHT: f32 = 20.0;
const CHROME_HEIGHT: f32 = 220.0;

#[component]
pub fn CompiledLogicView(editor: Editor) -> NodeId {
    let content = editor.block_content::<CompiledLogicContent>();
    let compiled = content.project(|content| content.root().compiled);
    let source = create_memo(move || {
        compiled.with(|compiled| {
            compiled
                .as_ref()
                .map(|compiled| ChildTarget::new(compiled.source(), LogicGridContent::CONTENT_TYPE))
        })
    });
    let calls = create_memo(move || {
        compiled.with(|compiled| {
            compiled
                .as_ref()
                .map(|compiled| compiled.calls().to_vec())
                .unwrap_or_default()
        })
    });
    let summary = create_memo(move || {
        compiled.with(|compiled| compiled.as_ref().map(summary_of).unwrap_or_default())
    });
    let ports = create_memo(move || -> Vec<String> {
        compiled.with(|compiled| {
            compiled
                .iter()
                .flat_map(|compiled| compiled.ports().iter().map(port_row))
                .collect()
        })
    });
    let program = create_memo(move || -> Vec<String> {
        compiled.with(|compiled| {
            compiled
                .iter()
                .flat_map(|compiled| compiled.program().instructions.iter().enumerate())
                .map(|(index, instruction)| format!("{index:>4}  {instruction}"))
                .collect()
        })
    });
    let no_calls = create_memo(move || calls.with(Vec::is_empty));
    create_effect(move || {
        let rows = ports.with(Vec::len) + calls.with(Vec::len) + program.with(Vec::len);
        let height = CHROME_HEIGHT + ROW_HEIGHT * rows as f32;
        editor.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    });

    view! {
        <Frame padding=PADDING>
            <List spacing=Space::None>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <List spacing=Space::Sm>
                            <Text style=TextStyle::HEADING>"Compiled from"</Text>
                            <BlockLink
                                editor={editor}
                                block={source}
                                @test_id={"compiled-logic.source"}
                            />
                            <Text style=TextStyle::CAPTION>{summary}</Text>
                        </List>
                        <Separator />
                        <List spacing=Space::Sm>
                            <Text style=TextStyle::HEADING>"Ports"</Text>
                            <Lines lines={ports} />
                        </List>
                        <Separator />
                        <List spacing=Space::Sm>
                            <Text style=TextStyle::HEADING>"Calls"</Text>
                            <Show condition={no_calls}>
                                <Text style=TextStyle::CAPTION>
                                    "This component calls nothing else."
                                </Text>
                            </Show>
                            <Calls editor={editor} calls={calls} />
                        </List>
                        <Separator />
                        <List spacing=Space::Sm>
                            <Text style=TextStyle::HEADING>"Program"</Text>
                            <Lines lines={program} monospace=true />
                        </List>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn Calls(editor: Editor, calls: Memo<Vec<Uuid>>) -> NodeId {
    let keys = create_memo(move || calls.get());
    view! {
        <List spacing=Space::Xs>
            <ForEach keys={keys}>|called: Uuid| {
                let block = Some(ChildTarget::new(called, CompiledLogicContent::CONTENT_TYPE));
                view! {
                    <BlockLink
                        editor={editor}
                        block={block}
                        @test_id={format!("compiled-logic.call.{called}")}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn Lines(lines: Memo<Vec<String>>, #[prop(default = false)] monospace: bool) -> NodeId {
    let keys = create_memo(move || (0..lines.with(Vec::len)).collect::<Vec<usize>>());
    view! {
        <List spacing=Space::Xs2>
            <ForEach keys={keys}>|index: usize| {
                let line = create_memo(move || {
                    lines.with(|lines| lines.get(index).cloned().unwrap_or_default())
                });
                view! {
                    <Text
                        size=TextSize::Sm
                        family={if monospace { Family::Mono } else { Family::Sans }}
                    >
                        {line}
                    </Text>
                }
            }</ForEach>
        </List>
    }
}

fn summary_of(compiled: &Program) -> String {
    let size = compiled.size();
    format!(
        "{} x {}   memory {}   storage {}",
        size.width,
        size.height,
        compiled.program().memory_size,
        compiled.program().storage_init.len()
    )
}

fn port_row(port: &ComponentPort) -> String {
    let direction = match port.direction {
        ConnectionDirection::Input => "in",
        ConnectionDirection::Output => "out",
    };
    let named = match port.label.is_empty() {
        true => format!("{direction} {}", port.index),
        false => format!("{direction} {} - {}", port.index, port.label),
    };
    format!("{named}   {:?}, {} bit", port.side, port.scale.get())
}
