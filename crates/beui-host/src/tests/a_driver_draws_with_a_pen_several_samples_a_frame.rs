use super::*;
use crate::input::PointerPress;
use crate::native::Document;
use crate::primitives::*;
use crate::reactive::*;

struct Counted {
    document: Document,
    frames: Signal<u32>,
}

impl App for Counted {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.frames.update(|frames| *frames += 1);
        self.document.show(context, rect);
    }
}

#[test]
fn a_driver_draws_with_a_pen_several_samples_a_frame() {
    let _root = crate::reactive::RootScope::new();
    let frames = create_signal(0_u32);
    let drags = create_signal::<Vec<(f32, u32)>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Interactive
                on_drag={move |press: PointerPress| {
                    let frame = frames.get_untracked();
                    drags.update(|drags| drags.push((press.pos.x, frame)));
                }}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut driven = stepped(Counted { document, frames });

    driven
        .ask(&["pen", "draw", "50,100", "250,100", "12", "4"])
        .expect("the pen draws");
    let drags = drags.get_untracked();
    let mut in_each_frame: Vec<usize> = Vec::new();
    let mut last = None;
    for (_, frame) in &drags {
        if last != Some(*frame) {
            in_each_frame.push(0);
            last = Some(*frame);
        }
        if let Some(count) = in_each_frame.last_mut() {
            *count += 1;
        }
    }
    assert_eq!(
        in_each_frame,
        vec![1, 4, 4, 4],
        "the press drags in a frame of its own, then each of the twelve samples, four in each frame: {drags:?}"
    );
    assert_eq!(drags.last().map(|(x, _)| *x), Some(250.0), "{drags:?}");

    assert!(
        driven
            .ask(&["pen", "draw", "50,100", "250,100", "12", "0"])
            .is_err(),
        "a frame holds at least one sample"
    );
}
