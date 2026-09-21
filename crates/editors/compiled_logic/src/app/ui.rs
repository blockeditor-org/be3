use block::Block;
use block_client::blocks::compiled_logic::CompiledLogic;
use block_client::blocks::logic_grid::LogicGrid;
use block_editor_plugin::beui::reactive::{
    ForEach, Frame, ItemSize, List, ReadSignal, Show, Text, clone, component, create_effect,
    create_memo, view,
};
use block_editor_plugin::beui::styled::theme::FONT_SMALL;
use block_editor_plugin::beui::styled::{Caption, Heading, Scroll, Separator, use_theme};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{BlockLink, ChildTarget, Editor};
use logicgame::grid::{ComponentPort, ConnectionDirection};
use uuid::Uuid;

use super::format_instruction;

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 16.0;
const INTRINSIC_WIDTH: f32 = 640.0;
const ROW_HEIGHT: f32 = 20.0;
const CHROME_HEIGHT: f32 = 220.0;

#[component]
pub fn CompiledLogicView(editor: Editor) -> NodeId {
    let compiled = editor.block::<CompiledLogic>();
    let source =
        compiled.project(|compiled| Some(ChildTarget::new(compiled.source(), LogicGrid::TYPE_ID)));
    let calls = compiled.project(|compiled| compiled.calls().to_vec());
    let summary = compiled.project(|compiled| {
        let size = compiled.size();
        format!(
            "{} x {}   memory {}   storage {}",
            size.width,
            size.height,
            compiled.program().memory_size,
            compiled.program().storage_init.len()
        )
    });
    let ports = compiled
        .project(|compiled| -> Vec<String> { compiled.ports().iter().map(port_row).collect() });
    let program = compiled.project(|compiled| -> Vec<String> {
        compiled
            .program()
            .instructions
            .iter()
            .enumerate()
            .map(|(index, instruction)| format!("{index:>4}  {}", format_instruction(instruction)))
            .collect()
    });
    let no_calls = create_memo(clone!(calls -> move || calls.with(Vec::is_empty)));
    let sized = editor.clone();
    create_effect(clone!(ports calls program -> move || {
        let rows = ports.with(Vec::len) + calls.with(Vec::len) + program.with(Vec::len);
        let height = CHROME_HEIGHT + ROW_HEIGHT * rows as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let source_editor = editor.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Heading content="Compiled from" />
                            <BlockLink
                                editor={source_editor}
                                block={source}
                                @test_id={"compiled-logic.source"}
                            />
                            <Caption content={summary} />
                        </List>
                        <Separator />
                        <List spacing=6.0>
                            <Heading content="Ports" />
                            <Lines lines={ports} />
                        </List>
                        <Separator />
                        <List spacing=6.0>
                            <Heading content="Calls" />
                            <Show condition={no_calls}>
                                <Caption content="This component calls nothing else." />
                            </Show>
                            <Calls editor={editor} calls={calls} />
                        </List>
                        <Separator />
                        <List spacing=6.0>
                            <Heading content="Program" />
                            <Lines lines={program} monospace=true />
                        </List>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn Calls(editor: Editor, calls: ReadSignal<Vec<Uuid>>) -> NodeId {
    let keys = create_memo(clone!(calls -> move || calls.get()));
    view! {
        <List spacing=4.0>
            <ForEach keys={keys}>
                {move |called: Uuid| {
                    let block = Some(ChildTarget::new(called, CompiledLogic::TYPE_ID));
                    view! {
                        <BlockLink
                            editor={editor.clone()}
                            block={block}
                            @test_id={format!("compiled-logic.call.{called}")}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn Lines(lines: ReadSignal<Vec<String>>, #[prop(default = false)] monospace: bool) -> NodeId {
    let keys =
        create_memo(clone!(lines -> move || (0..lines.with(Vec::len)).collect::<Vec<usize>>()));
    let theme = use_theme();
    view! {
        <List spacing=2.0>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let line = create_memo(clone!(lines -> move || {
                        lines.with(|lines| lines.get(index).cloned().unwrap_or_default())
                    }));
                    view! {
                        <Text
                            string={line}
                            font_size=FONT_SMALL
                            color={theme.text.clone()}
                            monospace={monospace}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
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
