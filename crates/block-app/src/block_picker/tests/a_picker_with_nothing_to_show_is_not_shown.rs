use super::*;

#[test]
fn a_picker_with_nothing_to_show_is_not_shown() {
    let closed = PickerView {
        create: None,
        ..creating(Uuid::new_v4(), 0)
    };

    publish(closed);

    assert!(views().is_empty());
}
