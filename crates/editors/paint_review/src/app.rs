use std::rc::Rc;

use block_client::blocks::paint_review::PaintReview;
use block_editor_plugin::beui::reactive::{
    Direction, ItemSize, List, component, create_memo, view,
};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor, Side, Sidebar};
use uuid::Uuid;

use crate::download::Source;

mod sidebar;
pub mod stage;
pub mod state;
mod toolbar;

pub use state::{Entry, Showing, Status};

use sidebar::PaintingList;
use stage::Stage;
use state::Review;
use toolbar::{FrameControls, ReviewCaption, ReviewToolbar};

const INTRINSIC_SIZE: Vec2 = Vec2::new(960.0, 640.0);
const SIDEBAR_WIDTH: f32 = 300.0;

pub struct PaintReviewApp;

impl block_editor_plugin::BeuiApp for PaintReviewApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PaintReviewEditor editor={editor} source=Source::Branch />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(PaintReview::new()).id())
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

#[component]
pub fn PaintReviewEditor(editor: Editor, source: Source) -> NodeId {
    let review = Review::new(&editor, source);
    let polled = Rc::clone(&review);
    editor.each_frame(move || polled.poll());

    let counted = Rc::clone(&review);
    let count = create_memo(move || {
        let Some(path) = counted.selected.get() else {
            return 1;
        };
        let Some(status) = counted.status(&path) else {
            return 1;
        };
        let _ = counted.revision.get();
        counted.count(&path, counted.shown_as(status))
    });

    let chrome = editor.chrome_shown();
    let bar_chrome = chrome.clone();
    let bar = Rc::clone(&review);
    let listed = Rc::clone(&review);
    let captioned = Rc::clone(&review);
    let framed = Rc::clone(&review);
    view! {
        <List spacing=0.0>
            <ReviewToolbar review={bar} shown={bar_chrome} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <Sidebar side=Side::Left shown={chrome} width=SIDEBAR_WIDTH>
                    <PaintingList review={listed} />
                </Sidebar>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <ReviewCaption review={captioned} />
                    <FrameControls review={framed} count={count.clone()} />
                    <Stage
                        @sizing=ItemSize::Percent(100.0)
                        @test_id={"paint_review.stage"}
                        review={review}
                        count={count}
                    />
                </List>
            </List>
        </List>
    }
}
