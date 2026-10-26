use super::*;
use crate::datetime::Time;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TimeList, TimeOptionHandle};

#[test]
fn a_time_list_moves_through_its_times_and_picks_one() {
    let _root = crate::reactive::RootScope::new();
    let picked = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <List @sizing=ItemSize::Fixed(200.0) spacing=0.0>
                    <TimeList
                        @sizing=ItemSize::Percent(100.0)
                        value={Binding::new(Some(Time::new(10, 7)), move |time| picked.update(|picked| picked.push(time)))}
                        step_minutes=30
                        focused=true
                        row_height=32.0
                        option={|handle: TimeOptionHandle| view! {
                            <Text>{handle.label}</Text>
                        }}
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

    assert_eq!(
        picked.get_untracked(),
        [Some(Time::new(10, 0)), Some(Time::new(12, 0))]
    );
}
