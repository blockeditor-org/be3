use be_type_text::TextContent;
#[cfg(test)]
mod tests;

pub(crate) mod embeds;
mod import_error;
mod large_embed;
mod position;
pub(crate) mod state;
mod surface;
mod toolbar;

use be_sdk_beui::beui::dock::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockTab, Docking, DockingLayout,
};
use be_sdk_beui::{Creation, Editor, Loaded};
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use uuid::Uuid;

use crate::hex::HexView;

use import_error::ImportError;
use state::State;
use surface::TextSurface;
use toolbar::{EditorMenu, FormatBar, format_actions, format_bar_shown};

const TEXT: &str = "text";
const FORMAT: &str = "format";

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
    editor.on_replace_child(move |old, new| state.is_alive() && state.replace_child(old, new));

    let content = NodeRef::new();
    editor.content(&content);
    action_scope(&content);
    let formats = format_actions(state);
    let format_bar = format_bar_shown(state);
    let layout = DockingLayout::<&'static str>::new();

    let hex = state.hex_view;
    let text_shown = create_memo(move || !hex.get());
    let hex_shown = create_memo(move || hex.get());
    let text_surface = move || {
        view! {
            <ListItem grow=1.0>
                <TextSurface state={state} />
            </ListItem>
        }
    };
    let hex_surface = move || {
        view! {
            <ListItem grow=1.0>
                <HexView state={state} />
            </ListItem>
        }
    };
    let format_bar_body = move || {
        let formats = formats.clone();
        view! {
            <DockBar
                id=FORMAT
                parent=TEXT
                placement={DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Start)}
            >
                <FormatBar state={state} formats={formats.clone()} />
            </DockBar>
        }
    };
    view! {
        <Docking layout focus={Some(TEXT)}>
            <DockPane id="text" locked=true bare=true>
                <DockTab id=TEXT title="Text">
                    <List @node_ref=&content spacing=Space::None>
                        <EditorMenu state={state} />
                        <ImportError state={state} />
                        <ListItem grow=1.0>
                            <Loaded label="Loading the text">|| {
                                editor.block_content::<TextContent>();
                                view! {
                                    <List spacing=Space::None>
                                        <Show condition={text_shown} then={text_surface} />
                                        <Show condition={hex_shown} then={hex_surface} />
                                    </List>
                                }
                            }</Loaded>
                        </ListItem>
                    </List>
                </DockTab>
            </DockPane>
            <Show condition={format_bar} then={format_bar_body} />
        </Docking>
    }
}
