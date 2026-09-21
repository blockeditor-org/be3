#[cfg(test)]
mod tests;

pub(crate) mod embeds;
mod find;
mod large_embed;
pub(crate) mod shapes;
pub(crate) mod state;
mod surface;
mod toolbar;

use beui::reactive::{
    ItemSize, List, NodeRef, Show, clone, component, create_effect, create_memo, create_signal,
    view,
};
use beui::{NodeId, Vec2};
use block_client::blocks::text::TextDocument;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

use crate::hex::{self, HexView};
use crate::layout::layout_document;

use find::{FindBar, ImportError};
use shapes::PADDING;
use state::{DIRECT_EDITOR_WIDTH, Shared, State};
use surface::TextSurface;
use toolbar::EditorToolbar;

pub struct TextApp;

impl block_editor_plugin::BeuiApp for TextApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <TextEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(TextDocument::new()).id())
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
            <FindBar state={state.clone()} />
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
    let content = state.content.clone();
    let hex = state.hex_view.clone();
    let measured = create_memo(clone!(state content hex width -> move || {
        content.get();
        let width = width.get();
        let embeds = state.embeds.get();
        let snapshot = state.snapshot.borrow();
        if !snapshot.loaded {
            return None;
        }
        if hex.get() {
            return Some(hex::intrinsic_size(snapshot.bytes.len(), width));
        }
        let document = layout_document(
            &snapshot.bytes,
            snapshot.highlight(),
            &embeds,
            &snapshot.checkbox_markers,
            &snapshot.hidden,
            (width - PADDING.x * 2.0).max(1.0),
        )?;
        Some(Vec2::new(width, document.size.y))
    }));
    let editor = state.editor.clone();
    create_effect(move || editor.set_intrinsic_size(measured.get()));
}
