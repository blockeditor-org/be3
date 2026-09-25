use block_editor_plugin::be_block::BlockContent;

use block_editor_plugin::be_block::compiled_logic::{CompiledLogic, CompiledLogicDocument};
use block_editor_plugin::be_block::{CompiledLogicContent, LogicGridContent};
use block_editor_plugin::{BlockInfo, BlockParent, Editor, EditorHost};
use block_ui_test::BeuiTest;
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

fn editor_on(source: Uuid, name: Option<&str>) -> BeuiTest<CompiledLogicApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    let mut grid = BlockInfo::new(source, LogicGridContent::CONTENT_TYPE, BlockParent::Root);
    grid.name = name.map(str::to_owned);
    grid.named_by_hand = name.is_some();
    editor.store().add_block(grid);
    editor.hold(
        None,
        CompiledLogicContent::new(&CompiledLogicDocument::of(compiled(source))),
    );
    editor.run();
    editor.run();
    editor
}

fn editor() -> BeuiTest<CompiledLogicApp> {
    editor_on(Uuid::new_v4(), None)
}
