#[cfg(test)]
mod tests;

pub(crate) mod embeds;
mod import_error;
mod large_embed;
pub(crate) mod state;
mod surface;
mod toolbar;

use beui::NodeId;
use beui::reactive::{
    ItemSize, List, NodeRef, Show, clone, component, create_effect, create_memo, create_signal,
    view,
};
use block_editor_beui::be_block::TextContent;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

use crate::hex::{self, HexView};

use embeds::ResolvedEmbed;
use import_error::ImportError;
use state::{DIRECT_EDITOR_WIDTH, Shared, State};
use surface::TextSurface;
use toolbar::EditorToolbar;

pub struct TextApp;

impl block_editor_beui::BeuiApp for TextApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <TextEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&TextContent::default()))
    }
}

#[component]
pub fn TextEditor(editor: Editor) -> NodeId {
    let state = State::new(editor.clone());
    let replacing = state.clone();
    editor.on_replace_child(move |old, new| replacing.replace_child(old, new));

    let content = NodeRef::new();
    editor.content(&content);
    intrinsic_size(&state);

    let hex = state.hex_view.clone();
    let text_shown = create_memo(clone!(hex -> move || !hex.get()));
    let hex_shown = create_memo(clone!(hex -> move || hex.get()));
    let surface_state = state.clone();
    let text_surface = move || {
        let state = surface_state.clone();
        view! {
            <TextSurface @sizing=ItemSize::Percent(100.0) state={state} />
        }
    };
    let hex_state = state.clone();
    let hex_surface = move || {
        let state = hex_state.clone();
        view! {
            <HexView @sizing=ItemSize::Percent(100.0) state={state} />
        }
    };
    view! {
        <List @node_ref=&content spacing=0.0>
            <EditorToolbar state={state.clone()} />
            <ImportError state={state.clone()} />
            <Show condition={text_shown} then={text_surface} />
            <Show condition={hex_shown} then={hex_surface} />
        </List>
    }
}

fn intrinsic_size(state: &Shared) {
    let (width, set_width) = create_signal(DIRECT_EDITOR_WIDTH);
    let resized = state.editor.resized();
    create_effect(move || {
        if let Some(size) = resized.get() {
            set_width.set(size.x.max(1.0));
        }
    });
    let content = state.text.content();
    let hex = state.hex_view.clone();
    let embeds = state.embeds.clone();
    let measured = create_memo(clone!(state content hex width embeds -> move || {
        content.get();
        let width = width.get();
        if !state.text.loaded() {
            return None;
        }
        if hex.get() {
            return Some(hex::intrinsic_size(state.text.bytes().len(), width));
        }
        let widgets = embeds
            .get()
            .iter()
            .map(ResolvedEmbed::widget)
            .collect::<Vec<_>>();
        state.text.measure(&widgets, width)
    }));
    let editor = state.editor.clone();
    create_effect(move || editor.set_intrinsic_size(measured.get()));
}
