use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_driver_clicks_a_button_by_its_label_and_hears_what_changed() {
    let document = build_node(|| {
        let added = create_signal(false);
        view! {
            <List spacing=8.0>
                <Button
                    label="Add"
                    variant=ButtonVariant::Primary
                    on_click={move || added.set(true)}
                />
                <ShowKeepAlive condition={added}>
                    <Text>"Added"</Text>
                </ShowKeepAlive>
            </List>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    let changed = driven
        .ask(&["click", "\"Add\""])
        .expect("the button and its label are one target");
    assert!(
        changed
            .lines()
            .any(|line| line.starts_with('+') && line.contains("Label value=\"Added\"")),
        "the answer names the label the click showed:\n{changed}"
    );

    let missing = driven
        .ask(&["click", "\"Remove\""])
        .expect_err("nothing is called Remove");
    assert!(missing.contains("no line of the tree"), "{missing}");
}
