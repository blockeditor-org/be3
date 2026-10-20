use block_editor_beui::be_block::PaintReviewContent;

use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{Creation, Editor, Side, Sidebar};
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

impl block_editor_beui::BeuiApp for PaintReviewApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::PaintReviewContent>(&[]);
        view! {
            <Loaded loaded={loaded}>
                <PaintReviewEditor editor={editor} source=Source::Branch />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&PaintReviewContent::default()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

#[component]
pub fn PaintReviewEditor(editor: Editor, source: Source) -> NodeId {
    let review = Review::new(editor, source);
    review.watch();

    let count = create_memo(move || {
        let Some(path) = review.selected.get() else {
            return 1;
        };
        let Some(status) = review.status(&path) else {
            return 1;
        };
        let _ = review.revision.get();
        review.count(&path, review.shown_as(status))
    });

    let chrome = editor.chrome_shown();
    view! {
        <List spacing=0.0>
            <ReviewToolbar review={review} shown={chrome} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <Sidebar side=Side::Left shown={chrome} width=SIDEBAR_WIDTH>
                    <PaintingList review={review} />
                </Sidebar>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <ReviewCaption review={review} />
                    <FrameControls review={review} count={count} />
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
