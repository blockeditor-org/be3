use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::*;
use crate::primitives::*;
use crate::reactive::*;

/// A kept `Embed` that a timer due in a frame moves to another place after the frame's first
/// layout (the timer removes its place and shows the other in one update, and the frame lays
/// out again) is told only where the frame leaves it: never unplaced on the way, so its app
/// does not hide and present again.
#[test]
fn a_kept_embed_moved_by_a_timer_is_told_only_where_the_frame_leaves_it() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let heard = Rc::new(RefCell::new(Vec::<Option<f32>>::new()));
    let first = create_signal(true);
    let second = create_signal(false);
    let tall = create_signal(100.0_f32);
    let heard_by = Rc::clone(&heard);
    let document = build_node(move || {
        let heard = heard_by;
        // Started once the embed is laid out lower, during the frame's layout: due at once, it
        // fires after that layout and moves the embed.
        let timer = create_timer(move || {
            first.set(false);
            second.set(true);
            None
        });
        {
            let heard = Rc::clone(&heard);
            slot.on_place(move |placement| {
                let top = placement.map(|placement| placement.rect.min.y);
                heard.borrow_mut().push(top);
                if top.is_some_and(|top| top >= 100.0) && first.get_untracked() {
                    timer.start(Duration::ZERO);
                }
            });
        }
        let kept = move || {
            view! {
                <Kept>
                    <Frame height=100.0>
                        <Embed slot={slot} />
                    </Frame>
                </Kept>
            }
        };
        view! {
            <List spacing=0.0>
                <Frame height={tall} />
                <KeepAlive>
                    <Show value={first}>
                        <Frame>{kept()}</Frame>
                    </Show>
                    <Show value={second}>
                        <Frame>{kept()}</Frame>
                    </Show>
                </KeepAlive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(*heard.borrow(), [Some(100.0)], "placed in its first place");
    heard.borrow_mut().clear();

    tall.set(150.0);
    harness.frame(Vec::new());
    assert!(!first.get_untracked(), "the timer fired in the frame");
    assert!(
        heard.borrow().iter().all(Option::is_some),
        "never heard unplaced on the way: {:?}",
        heard.borrow()
    );
    assert!(second.get_untracked());
    assert!(slot.placement().is_some());
}
