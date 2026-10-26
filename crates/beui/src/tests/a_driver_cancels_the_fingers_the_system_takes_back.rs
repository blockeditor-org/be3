use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_cancels_the_fingers_the_system_takes_back() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Button
                    label="Press"
                    variant=ButtonVariant::Primary
                    on_click={move || clicks.update(|clicks| *clicks += 1)}
                />
            </List>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    driven.run(&["app", "touch", "down", "0=\"Press\""]);
    driven.run(&["app", "touch", "cancel"]);
    assert_eq!(
        clicks.get_untracked(),
        0,
        "a finger the system takes back does not click"
    );
    driven.run(&["app", "touch", "down", "0=\"Press\""]);
    driven.run(&["app", "touch", "up"]);
    assert_eq!(
        clicks.get_untracked(),
        1,
        "the cancelled finger is no longer down, so it can press again"
    );
}
