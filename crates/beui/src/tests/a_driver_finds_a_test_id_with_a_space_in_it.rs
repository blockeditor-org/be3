use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_driver_finds_a_test_id_with_a_space_in_it() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Button
                    @test_id={"save it"}
                    label="Save"
                    variant=ButtonVariant::Primary
                    on_click={move || clicks.update(|clicks| *clicks += 1)}
                />
            </List>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    driven.run(&["click", "#save it"]);

    assert_eq!(
        clicks.get_untracked(),
        1,
        "a test id is found whole, though it has a space in it"
    );
}
