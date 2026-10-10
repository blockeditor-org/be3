use super::*;
use crate::reactive::{List, ShowKeepAlive, Text, build, create_signal, view};
use crate::styled::{Button, ButtonVariant};

#[test]
fn a_driver_clicks_a_button_by_its_label_and_hears_what_changed() {
    let document = build(|| {
        let (added, set_added) = create_signal(false);
        view! {
            <List spacing=8.0>
                <Button
                    label="Add"
                    variant=ButtonVariant::Primary
                    on_click={move || set_added.set(true)}
                />
                <ShowKeepAlive condition={added}>
                    <Text string="Added" />
                </ShowKeepAlive>
            </List>
        }
    });
    let mut driven = Driven::new(document);

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
