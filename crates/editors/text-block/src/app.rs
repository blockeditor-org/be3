#[cfg(test)]
mod tests;

pub(crate) mod embeds;
mod import_error;
mod large_embed;
pub(crate) mod state;
mod surface;
mod toolbar;

use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_editor_beui::be_block::TextContent;
use block_editor_beui::{Creation, Editor, Loaded};
use uuid::Uuid;

use crate::hex::{self, HexView};

use embeds::ResolvedEmbed;
use import_error::ImportError;
use state::{DIRECT_EDITOR_WIDTH, State};
use surface::TextSurface;
use toolbar::{EditorMenu, FormatBar, format_actions};

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
    let state = State::new(editor);
    editor.on_replace_child(move |old, new| state.is_alive() && state.replace_child(old, new));

    let content = NodeRef::new();
    editor.content(&content);
    action_scope(&content);
    intrinsic_size(state);
    let formats = format_actions(state);

    let hex = state.hex_view;
    let loaded = state.loaded;
    let text_shown = create_memo(move || !hex.get());
    let hex_shown = create_memo(move || hex.get());
    let text_surface = move || {
        view! {
            <TextSurface @sizing=ItemSize::Percent(100.0) state={state} />
        }
    };
    let hex_surface = move || {
        view! {
            <HexView @sizing=ItemSize::Percent(100.0) state={state} />
        }
    };
    view! {
        <List @node_ref=&content spacing=0.0>
            <EditorMenu state={state} />
            <ImportError state={state} />
            <Loaded @sizing=ItemSize::Percent(100.0) loaded={loaded} label="Loading the text">
                <List spacing=0.0>
                    <Show condition={text_shown} then={text_surface} />
                    <Show condition={hex_shown} then={hex_surface} />
                </List>
            </Loaded>
            <FormatBar state={state} formats />
        </List>
    }
}

fn intrinsic_size(state: State) {
    let (width, set_width) = create_signal(DIRECT_EDITOR_WIDTH);
    let resized = state.editor.resized();
    create_effect(move || {
        if let Some(size) = resized.get() {
            set_width.set(size.x.max(1.0));
        }
    });
    let content = state.text.content();
    let hex = state.hex_view;
    let embeds = state.embeds;
    let measured = create_memo(move || {
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
    });
    let editor = state.editor;
    create_effect(move || editor.set_intrinsic_size(measured.get()));
}
