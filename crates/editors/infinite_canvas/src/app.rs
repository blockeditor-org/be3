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
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod toolbar;

use canvas::CanvasStage;
use sidebar::CanvasSidebar;
use state::CanvasState;
use toolbar::CanvasToolbar;

use crate::geometry::{MIN_SIZE, preview_region_for_entities};

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
    let polled = Rc::clone(&state);
    editor.each_frame(move || polled.poll());

    let replacing = Rc::clone(&state);
    editor.on_replace_child(move |old, new| replacing.replace_referenced_block(old, new));

    report_intrinsic_size(&editor, &state);

    let content = NodeRef::new();
    editor.content(&content);
    let chrome = editor.chrome_shown();
    let bar = Rc::clone(&state);
    let stage = Rc::clone(&state);
    view! {
        <List spacing=0.0>
            <CanvasToolbar state={bar} shown={chrome.clone()} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <CanvasStage @sizing=ItemSize::Percent(100.0) @node_ref={&content} state={stage} />
                <CanvasSidebar state={state} shown={chrome} />
            </List>
        </List>
    }
}

#[component]
fn CanvasPreview(editor: Editor) -> NodeId {
    let state = CanvasState::new(&editor, true);
    let polled = Rc::clone(&state);
    editor.each_frame(move || polled.poll());
    report_intrinsic_size(&editor, &state);
    view! {
        <CanvasStage state={state} />
    }
}

fn report_intrinsic_size(editor: &Editor, state: &Rc<CanvasState>) {
    let sized = Rc::clone(state);
    let sizing = editor.clone();
    create_effect(move || {
        let region = sized
            .preview_region
            .get()
            .unwrap_or_else(|| preview_region_for_entities(&sized.entities.get()));
        sizing.set_intrinsic_size(Some(Vec2::new(
            region.size.x.max(MIN_SIZE),
            region.size.y.max(MIN_SIZE),
        )));
    });
}
