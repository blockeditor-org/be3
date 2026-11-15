use be_type_text::TextContent;
#[cfg(test)]
mod tests;

pub(crate) mod embeds;
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

    let content = NodeRef::new();
    editor.content(&content);
    action_scope(&content);
    let formats = format_actions(state);

    let format_bar = format_bar_shown(state);
    let layout = DockingLayout::<&'static str>::new();

    let text_surface = move || {
        view! {
            <TextSurface state={state} />
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
                        <ListItem grow=1.0>
                            <Loaded label="Loading the text">|| {
                                let _ = untrack(|| editor.block_content::<TextContent>().ready());
                                view! {
                                    <Show value={state.hex_view} fallback={text_surface}>
                                        <HexView state={state} />
                                    </Show>
                                }
                            }</Loaded>
                        </ListItem>
                    </List>
                </DockTab>
            </DockPane>
            <Show value={format_bar} then={move |_| format_bar_body()} />
        </Docking>
    }
}
