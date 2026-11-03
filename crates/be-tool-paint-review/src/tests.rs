use std::cell::RefCell;
use std::rc::Rc;

use be_sdk_test::DocumentTest;
use beui::{Image, Input, Modifiers, Vec2};

use super::*;
use be_tool_paint_review_manifest::{Frame, Manifest, Painting, Picture, Status};

mod a_missing_image_says_why;
mod a_phone_shows_the_grid;
mod a_recording_steps_through_the_frames_that_changed;
mod actual_size_uses_image_pixels_even_on_a_dense_screen;
mod an_image_that_has_not_arrived_shows_it_is_loading;
mod an_opened_painting_compares_before_and_after;
mod an_unreadable_painting_says_why;
mod dragging_the_line_moves_the_split;
mod escape_returns_to_the_grid;
mod holding_shift_shows_the_changes;
mod holding_space_shows_what_the_paintings_were;
mod hovering_a_painting_shows_it_at_full_size;
mod hovering_fetches_the_paintings_nearby;
mod j_and_k_move_through_the_paintings_in_order;
mod only_thumbnails_are_fetched_for_the_grid;
mod the_grid_shows_every_painting;
mod the_image_can_return_to_fit_after_zooming;
mod the_manifest_shows_it_is_loading_until_it_arrives;
mod the_manifest_that_failed_says_why;
mod the_painting_named_in_the_address_is_opened;

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
        let app = PaintReviewProgram::new(painting.map(str::to_owned), move |request| {
            asked.borrow_mut().push(request)
        });
        let manifest = app.manifest();
        let PaintReviewProgram {
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

    fn hold(&mut self, key: beui::Key, pressed: bool) {
        self.test.frame(vec![Input::Key(beui::Keystroke {
            key: Some(key),
            pressed,
            ..Default::default()
        })]);
        self.test.frame(Vec::new());
    }

    fn shift(&mut self, held: bool) {
        self.test.frame(vec![Input::Modifiers(Modifiers {
            shift: held,
            ..Modifiers::NONE
        })]);
        self.test.frame(Vec::new());
    }

    fn text(&self, test_id: &str) -> String {
        let document = self.test.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing is marked {test_id}"));
        document
            .text(document.arena.kind_of(node).expect("a text"))
            .to_owned()
    }

    fn camera(&self) -> beui::styled::PanZoomView {
        let document = self.test.document();
        document
            .api::<beui::styled::PanZoomApi>(
                document.find_test_id("paint_review.pan_zoom").unwrap(),
            )
            .view()
            .get()
    }
}

fn shot(hash: String) -> Picture {
    Picture {
        thumbnail: format!("small-{hash}"),
        image: hash,
        width: WIDTH,
        height: HEIGHT,
    }
}

fn frame(number: usize, before: Option<&str>, after: Option<&str>) -> Frame {
    let changes = before
        .zip(after)
        .map(|(before, _)| before.replace("before", "changes"));
    Frame {
        number,
        before: before.map(|hash| shot(hash.to_owned())),
        after: after.map(|hash| shot(hash.to_owned())),
        changed_pixels: changes.as_ref().map(|_| 1_536),
        changes: changes.map(shot),
    }
}

fn changed(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Changed,
        changed_pixels: 1_536,
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
        changed_pixels: 38_400,
        detail: String::new(),
        frames: vec![frame(1, None, Some(&format!("after-{name}")))],
    }
}

fn removed(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Removed,
        changed_pixels: 38_400,
        detail: String::new(),
        frames: vec![frame(1, Some(&format!("before-{name}")), None)],
    }
}

fn unreadable(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Unreadable,
        changed_pixels: 0,
        detail: "the painting is not a snapshot: unexpected end of file".to_owned(),
        frames: Vec::new(),
    }
}

fn recording(name: &str) -> Painting {
    Painting {
        name: name.to_owned(),
        status: Status::Changed,
        changed_pixels: 1_536,
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
            removed("checklist.an_old_list"),
            changed("beui-demo.buttons"),
            changed("text-block.typing_a_paragraph"),
            unreadable("map.broken"),
        ],
    }
}

fn picture(hash: &str) -> Result<Image, String> {
    if hash.contains("missing") {
        return Err("HTTP 404".to_owned());
    }
    if let Some(full) = hash.strip_prefix("small-") {
        return picture(full).map(|image| half(&image));
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

fn half(image: &Image) -> Image {
    let (width, height) = (image.width() / 2, image.height() / 2);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let at = ((y * 2 * image.width() + x * 2) * 4) as usize;
            pixels.extend_from_slice(&image.pixels()[at..at + 4]);
        }
    }
    Image::from_rgba(width, height, pixels)
}
