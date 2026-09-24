use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use block::BlockParent;
use block_client::BlockClient;
use block_client::block_ref::BlockRef;
use block_client::blocks::paint_review::PaintReview as ReviewBlock;
use block_client::blocks::paint_snapshot::PaintSnapshot;
use block_editor_plugin::be_block::paint::PaintReview;
use block_editor_plugin::be_block::{
    PaintReviewContent, PaintSnapshotContent, PaintSnapshotHeader,
};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness, ContentStore};
use paint_snapshot::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};
use uuid::Uuid;

use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

use crate::app::{PaintReviewApp, PaintReviewEditor, Status};
use crate::download::{Painting, Source};

mod a_painting_being_shown_is_not_evicted_for_another;
mod a_painting_is_only_rastered_once;
mod a_painting_that_changed_on_the_branch_is_modified;
mod a_painting_that_vanished_is_removed;
mod a_painting_that_was_never_approved_is_new;
mod a_recording_is_reviewed_one_frame_at_a_time;
mod a_tree_lists_only_the_paintings_on_it;
mod a_tree_that_says_nothing_useful_is_an_error;
mod choosing_a_painting_rasters_the_one_it_was_approved_as;
mod paintings_are_downloaded_a_few_at_a_time;
mod the_difference_counts_the_pixels_that_changed;
mod the_difference_shows_the_pixels_that_changed;
mod the_painting_can_be_zoomed_in_on;
mod unapproving_a_painting_makes_it_new_again;

const PATH: &str = "counter.a_button_is_drawn.paint";

struct Review {
    branch: Arc<Mutex<Vec<Painting>>>,
    client: Arc<BlockClient>,
    block: Uuid,
    store: ContentStore,
}

impl Review {
    fn open() -> (Self, ContentHarness<PaintReviewApp>) {
        let branch = Arc::new(Mutex::new(Vec::new()));
        let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
        let block = client.create_block(ReviewBlock::new());
        write_to(&branch, PATH, &painting(30));
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), Arc::clone(&client), block.id());
        let source = Source::Fixed(Arc::clone(&branch));
        let test = BeuiTest::<PaintReviewApp>::with_view(editor.clone(), move || {
            view! {
                <PaintReviewEditor editor={editor} source={source} />
            }
        })
        .in_viewport();
        let mut editor = ContentHarness::new(test, host);
        editor.hold(None, PaintReviewContent::default());
        let review = Self {
            branch,
            client,
            block: block.id(),
            store: editor.store(),
        };
        editor.run();
        editor.run();
        (review, editor)
    }

    fn review(&self) -> PaintReview {
        self.store.content::<PaintReviewContent>(None).root()
    }

    fn write(&self, path: &str, painting: &Snapshot) {
        write_to(&self.branch, path, painting);
    }

    fn approve(&self, path: &str, painting: &Snapshot) {
        let data = painting.encode().unwrap();
        let hash = PaintSnapshot::fingerprint(&data);
        let created = self.client.create_block(PaintSnapshot::new());
        created.set_parent(BlockParent::Uuid(self.block));
        self.store.hold(
            Some(created.id()),
            PaintSnapshotContent::new(
                PaintSnapshotHeader {
                    path: path.to_owned(),
                    hash: hash.clone(),
                },
                data,
            ),
        );
        self.store.edit::<PaintReviewContent>(
            None,
            &PaintReview::approve(path, hash, BlockRef::Direct(created.id())),
        );
    }

    fn remove(&self, path: &str) {
        self.branch.lock().unwrap().retain(|held| held.path != path);
    }

    fn approved(&self, path: &str) -> Option<PaintSnapshotContent> {
        let approval = self.review().approval(path).cloned()?;
        let id = approval.snapshot.as_direct()?;
        let snapshot = self.store.content::<PaintSnapshotContent>(Some(id));
        assert_eq!(approval.hash, snapshot.header().hash);
        assert_eq!(path, snapshot.header().path);
        Some(snapshot)
    }

    fn reference(&self, path: &str) -> Option<Uuid> {
        self.review().approval(path)?.snapshot.as_direct()
    }

    fn orphaned(&self, id: Uuid) -> bool {
        self.client
            .get_block::<PaintSnapshot>(id)
            .relationships()
            .parent
            == BlockParent::Orphaned
    }

    fn approvals(&self) -> usize {
        self.review().approved().len()
    }

    fn status(&self, path: &str) -> Option<Status> {
        let approved = self.review().approval(path).cloned();
        let branch = self.branch.lock().unwrap();
        let found = branch.iter().find(|painting| painting.path == path);
        match (found, approved) {
            (None, None) => None,
            (None, Some(_)) => Some(Status::Removed),
            (Some(_), None) => Some(Status::New),
            (Some(found), Some(approved)) => Some(match found.hash == approved.hash {
                true => Status::Unchanged,
                false => Status::Modified,
            }),
        }
    }
}

fn stage(editor: &BeuiTest<PaintReviewApp>) -> NodeId {
    editor
        .document()
        .find_test_id("paint_review.stage")
        .expect("the stage has not been built")
}

fn settled(editor: &mut BeuiTest<PaintReviewApp>) {
    let quiet = std::cell::Cell::new(0);
    editor.settle_until("the review to settle", |editor| {
        let resting = !crate::app::stage::busy(editor.document(), stage(editor))
            && !editor.shown("paint_review.notice");
        quiet.set(match resting {
            true => quiet.get() + 1,
            false => 0,
        });
        quiet.get() >= 3
    });
}

fn rasters(editor: &BeuiTest<PaintReviewApp>) -> usize {
    crate::app::stage::rastered(editor.document(), stage(editor))
}

fn shown_frame(editor: &BeuiTest<PaintReviewApp>) -> usize {
    crate::app::stage::frame_shown(editor.document(), stage(editor))
}

fn shown_zoom(editor: &BeuiTest<PaintReviewApp>) -> f32 {
    crate::app::stage::zoom(editor.document(), stage(editor))
}

fn painting(background: u8) -> Snapshot {
    recording(&[background])
}

fn recording(backgrounds: &[u8]) -> Snapshot {
    Snapshot {
        frames: backgrounds
            .iter()
            .map(|background| Frame {
                size: [24, 16],
                pixels_per_point: 1.0,
                background: [*background, *background, *background, 255],
                primitives: Vec::new(),
            })
            .collect(),
        textures: BTreeMap::new(),
    }
}

fn marked(background: u8, left: f32) -> Snapshot {
    let white = Texture::encode([1, 1], &[[255, 255, 255, 255]]).unwrap();
    let corner = |x: f32, y: f32| Vertex {
        pos: [x, y],
        uv: [0.5, 0.5],
        color: [220, 40, 60, 255],
    };
    Snapshot {
        frames: vec![Frame {
            size: [24, 16],
            pixels_per_point: 1.0,
            background: [background, background, background, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 24.0, 16.0],
                content: Content::Mesh(vec![Triangle {
                    texture: 0,
                    corners: [
                        corner(left, 3.0),
                        corner(left + 7.0, 3.0),
                        corner(left, 12.0),
                    ],
                }]),
            }],
        }],
        textures: BTreeMap::from([(0, white)]),
    }
}

fn write_to(branch: &Mutex<Vec<Painting>>, path: &str, painting: &Snapshot) {
    let data = painting.encode().unwrap();
    let mut branch = branch.lock().unwrap();
    branch.retain(|held| held.path != path);
    branch.push(Painting {
        path: path.to_owned(),
        hash: PaintSnapshot::fingerprint(&data),
        data,
    });
    branch.sort_by(|left, right| left.path.cmp(&right.path));
}

fn entry_id(path: &str) -> String {
    format!("paint_review.entry.{path}")
}
