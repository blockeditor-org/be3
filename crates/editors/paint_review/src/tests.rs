use block_editor_beui::be_block::BlockContent;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use block_editor_beui::be_block::paint::PaintReview;
use block_editor_beui::be_block::{PaintReviewContent, PaintSnapshotContent, PaintSnapshotHeader};
use block_editor_beui::{BlockInfo, BlockParent};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentStore};
use paint_snapshot::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};
use uuid::Uuid;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;

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
    block: Uuid,
    store: ContentStore,
}

impl Review {
    fn open() -> (Self, BeuiTest<PaintReviewApp>) {
        let branch = Arc::new(Mutex::new(Vec::new()));
        let block = Uuid::new_v4();
        write_to(&branch, PATH, &painting(30));
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), block);
        let source = Source::Fixed(Arc::clone(&branch));
        let test = BeuiTest::<PaintReviewApp>::with_view(editor.clone(), move || {
            view! {
                <PaintReviewEditor editor={editor} source={source} />
            }
        })
        .in_viewport();
        let mut editor = test;
        editor.hold(None, PaintReviewContent::default());
        let review = Self {
            branch,
            block,
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
        let hash = crate::download::fingerprint(&data);
        let created = Uuid::new_v4();
        self.store.add_block(BlockInfo::new(
            created,
            PaintSnapshotContent::CONTENT_TYPE,
            BlockParent::Block(self.block),
        ));
        self.store.hold(
            Some(created),
            PaintSnapshotContent::new(
                PaintSnapshotHeader {
                    path: path.to_owned(),
                    hash: hash.clone(),
                },
                data,
            ),
        );
        self.store
            .edit::<PaintReviewContent>(None, &PaintReview::approve(path, hash, created));
    }

    fn remove(&self, path: &str) {
        self.branch.lock().unwrap().retain(|held| held.path != path);
    }

    fn approved(&self, path: &str) -> Option<PaintSnapshotContent> {
        let approval = self.review().approval(path).cloned()?;
        let id = approval.snapshot;
        let snapshot = self.store.content::<PaintSnapshotContent>(Some(id));
        assert_eq!(approval.hash, snapshot.header().hash);
        assert_eq!(path, snapshot.header().path);
        Some(snapshot)
    }

    fn reference(&self, path: &str) -> Option<Uuid> {
        Some(self.review().approval(path)?.snapshot)
    }

    fn orphaned(&self, id: Uuid) -> bool {
        self.store
            .block(id)
            .is_some_and(|info| info.parent == BlockParent::Detached)
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
    editor.settle_until("the review to settle", |editor| {
        !crate::app::stage::busy(editor.document(), stage(editor))
            && !editor.shown("paint_review.notice")
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
        hash: crate::download::fingerprint(&data),
        data,
    });
    branch.sort_by(|left, right| left.path.cmp(&right.path));
}

fn entry_id(path: &str) -> String {
    format!("paint_review.entry.{path}")
}
