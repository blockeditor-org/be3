use text_block_types::TextContent;
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
use block_editor_beui::{Creation, Editor, Loaded};
use uuid::Uuid;

use crate::hex::HexView;

use import_error::ImportError;
use state::State;
use surface::TextSurface;
use toolbar::{EditorMenu, FormatBar, format_actions};

pub struct TextApp;

#[component]
impl block_editor_beui::BeuiApp for TextApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <TextEditor editor={editor} />
            </Root>
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
    let formats = format_actions(state);

    let hex = state.hex_view;
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
        <List @node_ref=&content spacing=Space::None>
            <EditorMenu state={state} />
            <ImportError state={state} />
            <Loaded @sizing=ItemSize::Percent(100.0) label="Loading the text">|| {
                editor.block_content::<TextContent>();
                view! {
                    <List spacing=Space::None>
                        <Show condition={text_shown} then={text_surface} />
                        <Show condition={hex_shown} then={hex_surface} />
                    </List>
                }
            }</Loaded>
            <FormatBar state={state} formats />
        </List>
    }
}
