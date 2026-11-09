use super::*;

#[test]
fn the_label_filter_is_a_key_and_a_value() {
    let content = edited(
        &BoatContent::default(),
        [
            Boat::set_organization(" acme "),
            Boat::set_name_prefix("ci-"),
            Boat::set_label(" env = prod "),
        ],
    );
    let boat = content.root();

    assert_eq!(boat.organization, "acme");
    assert_eq!(boat.name_prefix, "ci-");
    assert_eq!(Boat::label_filter(&boat.label), Some(("env", "prod")));

    let unlabelled = edited(&content, [Boat::set_label("prod")]).root();
    assert_eq!(Boat::label_filter(&unlabelled.label), None);
}
