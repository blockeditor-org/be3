use block_ui_test::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use beui::{Image, Input, Vec2};
use block_ui_test::DocumentTest;

use super::*;
use paint_review_manifest::{Frame, Manifest, Painting, Status};

mod a_changed_painting_shows_both_sides_by_side;
mod a_changed_painting_shows_its_changes_in_magenta;
mod a_changed_painting_shows_what_it_became;
mod a_changed_painting_shows_what_it_was;
mod a_missing_image_says_why;
mod a_phone_link_opens_the_named_painting;
mod a_phone_opens_a_painting_and_returns_to_the_list;
mod a_recording_steps_through_the_frames_that_changed;
mod a_removed_painting_has_only_a_before;
mod actual_size_uses_image_pixels_even_on_a_dense_screen;
mod an_added_painting_has_only_an_after;
mod an_image_that_has_not_arrived_shows_it_is_loading;
mod an_unreadable_painting_says_why;
mod j_and_k_move_through_the_paintings_in_the_order_listed;
mod only_the_frame_shown_is_fetched;
mod space_flips_between_before_and_after;
mod the_image_can_return_to_fit_after_zooming;
mod the_manifest_shows_it_is_loading_until_it_arrives;
mod the_manifest_that_failed_says_why;
mod the_painting_named_in_the_address_is_shown_first;

const WIDE: Vec2 = Vec2::new(1100.0, 760.0);
const WIDTH: u32 = 240;
const HEIGHT: u32 = 160;

struct Review {
    test: DocumentTest,
    manifest: Signal<Fetch<Rc<Manifest>>>,
    requests: Rc<RefCell<Vec<ImageRequest>>>,
    _app: (
        OwnedSignal<Fetch<Rc<Manifest>>>,
        OwnedStored<images::ImageTable>,
    ),
}

impl Review {
    fn open(painting: Option<&str>) -> Self {
        Self::sized(painting, WIDE)
    }

    fn sized(painting: Option<&str>, size: Vec2) -> Self {
        let requests: Rc<RefCell<Vec<ImageRequest>>> = Rc::default();
        let asked = requests.clone();
        let app = PaintReviewApp::new(painting.map(str::to_owned), move |request| {
            asked.borrow_mut().push(request)
        });
        let manifest = app.manifest();
        let PaintReviewApp {
            document,
            manifest: owned,
            images,
        } = app;
        Self {
            test: DocumentTest::new(document, size),
            manifest,
            requests,
            _app: (owned, images),
        }
    }

    fn with(manifest: Manifest) -> Self {
        let mut review = Self::open(None);
        review.load(manifest);
        review
    }

    fn load(&mut self, manifest: Manifest) {
        self.manifest.set(Fetch::Ready(Rc::new(manifest)));
        self.test.frame(Vec::new());
    }

    fn requested(&self) -> Vec<String> {
        self.requests
            .borrow()
            .iter()
            .map(|request| request.hash().to_owned())
            .collect()
    }

    fn deliver(&mut self) {
        let requests: Vec<ImageRequest> = self.requests.borrow_mut().drain(..).collect();
        for request in requests {
            let image = picture(request.hash());
            request.deliver(image);
        }
        self.test.frame(Vec::new());
    }

    fn press(&mut self, key: beui::Key) {
        self.test.frame(
            [true, false]
                .into_iter()
                .map(|pressed| {
                    Input::Key(beui::Keystroke {
                        key: Some(key),
                        pressed,
                        ..Default::default()
                    })
                })
                .collect(),
        );
        self.test.frame(Vec::new());
    }

    fn shown_name(&self) -> String {
        let document = self.test.document();
        let node = document
            .find_test_id("paint_review.name")
            .expect("a painting is shown");
        document
            .text(document.arena.kind_of(node).expect("the name is a text"))
            .to_owned()
    }

    fn pressed(&self, mode: &str) -> bool {
        let document = self.test.document();
        let node = document
            .find_test_id(&format!("paint_review.mode.{mode}"))
            .expect("the mode has a button");
        document
            .api::<beui::styled::ToggleApi>(node)
            .checked()
            .get()
    }
}

fn frame(number: usize, before: Option<&str>, after: Option<&str>) -> Frame {
    let changes = before
        .zip(after)
        .map(|(before, _)| before.replace("before", "changes"));
    Frame {
        number,
        before: before.map(str::to_owned),
        after: after.map(str::to_owned),
        changed_pixels: changes.as_ref().map(|_| 1_536),
        changes,
    }
}

fn changed(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Changed,
        detail: "frame 1 changed: 1536 pixels differ, a button moved down by 8 pixels".to_owned(),
        frames: vec![frame(
            1,
            Some(&format!("before-{name}")),
            Some(&format!("after-{name}")),
        )],
    }
}

fn added(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Added,
        detail: String::new(),
        frames: vec![frame(1, None, Some(&format!("after-{name}")))],
    }
}

fn removed(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Removed,
        detail: String::new(),
        frames: vec![frame(1, Some(&format!("before-{name}")), None)],
    }
}

fn unreadable(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Unreadable,
        detail: "the painting is not a snapshot: unexpected end of file".to_owned(),
        frames: Vec::new(),
    }
}

fn recording(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Changed,
        detail: "frames 2, 4 and 5 changed".to_owned(),
        frames: [2, 4, 5]
            .into_iter()
            .map(|number| {
                frame(
                    number,
                    Some(&format!("before-{name}-{number}")),
                    Some(&format!("after-{name}-{number}")),
                )
            })
            .collect(),
    }
}

fn mixed() -> Manifest {
    Manifest {
        paintings: vec![
            added("beui-demo.a_new_page"),
            changed("beui-demo.buttons"),
            removed("checklist.an_old_list"),
            unreadable("pixel-art.broken"),
            changed("text-block.typing_a_paragraph"),
        ],
    }
}

fn picture(hash: &str) -> Result<Image, String> {
    if hash.contains("missing") {
        return Err("HTTP 404".to_owned());
    }
    let shift = hash
        .rsplit('-')
        .next()
        .and_then(|number| number.parse::<u32>().ok())
        .unwrap_or(0);
    let top = |after: bool| 60 + shift * 4 + if after { 8 } else { 0 };
    let button = |after: bool, x: u32, y: u32| {
        (40..120).contains(&x) && (top(after)..top(after) + 24).contains(&y)
    };
    let after = hash.starts_with("after");
    let changes = hash.starts_with("changes");
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let card = (20..220).contains(&x) && (20..140).contains(&y);
            let pixel = match (changes, card) {
                (true, _) if button(false, x, y) != button(true, x, y) => [255, 0, 255, 255],
                (true, true) => [70, 70, 80, 255],
                (true, false) => [30, 30, 36, 255],
                (false, _) if button(after, x, y) => [64, 120, 230, 255],
                (false, true) => [235, 235, 240, 255],
                (false, false) => [40, 40, 48, 255],
            };
            pixels.extend(pixel);
        }
    }
    Ok(Image::from_rgba(WIDTH, HEIGHT, pixels))
}
