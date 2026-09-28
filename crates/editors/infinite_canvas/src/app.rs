use block_editor_beui::be_block::CanvasContent;
use std::rc::Rc;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::{
    Direction, ItemSize, List, NodeRef, component, create_effect, view,
};
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod components;
pub(crate) mod input;
pub(crate) mod menu;
pub(crate) mod overlay;
pub(crate) mod paint;
pub(crate) mod selection_bar;
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod toolbar;

use canvas::CanvasStage;
use selection_bar::SelectionBar;
use sidebar::CanvasSidebar;
use state::CanvasState;
use toolbar::{CanvasToolbar, ToolDock};

use crate::geometry::{MIN_SIZE, embedded_region};

pub struct CanvasApp;

impl block_editor_beui::BeuiApp for CanvasApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <CanvasEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <CanvasPreview editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CanvasContent::default()))
    }
}

#[component]
fn CanvasEditor(editor: Editor) -> NodeId {
    let state = CanvasState::new(&editor, false);
    state.watch();

    let replacing = Rc::clone(&state);
    editor.on_replace_child(move |old, new| replacing.replace_referenced_block(old, new));

    report_intrinsic_size(&editor, &state);

    let content = NodeRef::new();
    editor.content(&content);
    let chrome = editor.chrome_shown();
    let bar = Rc::clone(&state);
    let stage = Rc::clone(&state);
    let dock = Rc::clone(&state);
    let bar_state = Rc::clone(&state);
    let anchor = content.clone();
    let bar_anchor = content.clone();
    let docked = chrome.clone();
    let barred = chrome.clone();
    let side = chrome.clone();
    view! {
        <List spacing=0.0>
            <CanvasToolbar state={bar} shown={chrome.clone()} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <CanvasStage @sizing=ItemSize::Percent(100.0) @node_ref={&content} state={stage} />
                <CanvasSidebar state={state} shown={side} />
            </List>
            <ToolDock state={dock} anchor={anchor} shown={docked} />
            <SelectionBar state={bar_state} anchor={bar_anchor} shown={barred} />
        </List>
    }
}

#[component]
fn CanvasPreview(editor: Editor) -> NodeId {
    let state = CanvasState::new(&editor, true);
    state.watch();
    report_intrinsic_size(&editor, &state);
    view! {
        <CanvasStage state={state} />
    }
}

fn report_intrinsic_size(editor: &Editor, state: &Rc<CanvasState>) {
    let sized = Rc::clone(state);
    let sizing = editor.clone();
    create_effect(move || {
        let region = embedded_region(&sized.entities.get()).size();
        sizing.set_intrinsic_size(Some(Vec2::new(
            region.x.max(MIN_SIZE),
            region.y.max(MIN_SIZE),
        )));
    });
}
