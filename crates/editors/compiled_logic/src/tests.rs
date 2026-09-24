use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::compiled_logic::CompiledLogic as CompiledBlock;
use block_editor_plugin::be_block::CompiledLogicContent;
use block_editor_plugin::be_block::compiled_logic::{CompiledLogic, CompiledLogicDocument};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use logicgame::execution::{Instruction, UnlinkedComponent};
use logicgame::grid::{ComponentPort, ComponentSide, ConnectionDirection, Scale, Size};
use uuid::Uuid;

use crate::app::CompiledLogicApp;

mod the_program_lists_every_instruction;
mod the_source_link_names_the_grid_it_was_compiled_from;

fn compiled(source: Uuid) -> CompiledLogic {
    CompiledLogic::new(
        source,
        Size {
            width: 4,
            height: 3,
        },
        vec![
            ComponentPort {
                direction: ConnectionDirection::Input,
                index: 0,
                scale: Scale::ONE,
                side: ComponentSide::Left,
                start: 0,
                end: 1,
                label: "a".into(),
            },
            ComponentPort {
                direction: ConnectionDirection::Output,
                index: 0,
                scale: Scale::ONE,
                side: ComponentSide::Right,
                start: 0,
                end: 1,
                label: String::new(),
            },
        ],
        UnlinkedComponent {
            inputs: vec![0],
            outputs: vec![1],
            components: Vec::new(),
            instructions: vec![
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::SaveStorage {
                    storage: 0,
                    input: 1,
                },
            ],
            subgraphs: Vec::new(),
            memory_size: 2,
            storage_init: vec![0],
        },
    )
}

fn editor_on(client: Arc<BlockClient>, source: Uuid) -> ContentHarness<CompiledLogicApp> {
    let block = client.create_block(CompiledBlock::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut editor = ContentHarness::new(BeuiTest::new(editor), host);
    editor.hold(
        None,
        CompiledLogicContent::new(&CompiledLogicDocument::of(compiled(source))),
    );
    editor.run();
    editor.run();
    editor
}

fn editor() -> ContentHarness<CompiledLogicApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    editor_on(client, Uuid::new_v4())
}
