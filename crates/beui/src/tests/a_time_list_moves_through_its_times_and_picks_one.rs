use super::*;
use crate::datetime::Time;
use crate::reactive::{Text, view};
use crate::unstyled::{TimeList, TimeOptionHandle};

#[test]
fn a_time_list_moves_through_its_times_and_picks_one() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <List @sizing=ItemSize::Fixed(200.0) spacing=0.0>
                    <TimeList
                        @sizing=ItemSize::Percent(100.0)
                        value={Some(Time::new(10, 7))}
                        step_minutes=30
                        focused=true
                        option={|handle: TimeOptionHandle| view! {
                            <Text string={handle.label} />
                        }}
                        on_change={move |time| sink.borrow_mut().push(time)}
                    />
                </List>
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::PageDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*picked.borrow(), [Time::new(10, 0), Time::new(12, 0)]);
}
