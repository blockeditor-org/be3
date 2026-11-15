use be_type_text::TextContent;
#[cfg(test)]
mod tests;

pub(crate) mod embeds;
mod large_embed;
mod position;
pub(crate) mod state;
mod surface;
mod toolbar;

use be_sdk_beui::{Creation, Editor, Loaded};
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use uuid::Uuid;

use crate::hex::HexView;

use state::State;
use surface::TextSurface;
use toolbar::{EditorMenu, FormatBar, format_actions};

pub struct TextRoot;

#[component]
impl be_sdk_beui::BeuiRoot for TextRoot {
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
    position::keep_position(state);

    let content = NodeRef::new();
    editor.content(&content);
    action_scope(&content);
    let formats = format_actions(state);

    let text_surface = move || {
        view! {
            <TextSurface state={state} />
        }
    };
    view! {
        <List @node_ref=&content spacing=Space::None>
            <EditorMenu state={state} />
            <ListItem grow=1.0>
                <Loaded label="Loading the text">|| {
                    editor.block_content::<TextContent>();
                    view! {
                        <Show condition={state.hex_view} fallback={text_surface}>
                            <HexView state={state} />
                        </Show>
                    }
                }</Loaded>
            </ListItem>
            <FormatBar state={state} formats />
        </List>
    }
}
