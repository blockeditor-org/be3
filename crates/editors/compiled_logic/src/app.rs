use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use logicgame::execution::Instruction;

mod ui;

use ui::CompiledLogicView;

pub struct CompiledLogicApp;

impl block_editor_plugin::BeuiApp for CompiledLogicApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <CompiledLogicView editor={editor} />
        }
    }
}

pub fn format_instruction(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Call {
            component,
            instance,
            subgraph,
            inputs,
            outputs,
            ..
        } => format!("CALL c{component} i{instance} g{subgraph} {inputs:?} -> {outputs:?}"),
        Instruction::Not { input, output } => format!("NOT m{input} -> m{output}"),
        Instruction::CopyBits {
            input,
            output,
            shift,
            mask,
        } => format!("BITS m{input} shift {shift} mask {mask:#x} -> m{output}"),
        Instruction::ReadStorage { storage, output } => format!("READ s{storage} -> m{output}"),
        Instruction::SaveStorage { storage, input } => format!("SAVE m{input} -> s{storage}"),
    }
}
