use super::*;

#[test]
fn operating_through_the_source_is_visible_before_it_returns() {
    let client = client();
    let source = BlockSource::new(client.create_block(UiSettings::default()), || {});
    let zoom = source.project(UiSettings::zoom);

    source.operate(UiSettingsOperation::SetZoom { zoom: 1.5 });
    assert_eq!(zoom.get_untracked(), 1.5);
    source.operate(UiSettingsOperation::SetZoom { zoom: 0.75 });
    assert_eq!(zoom.get_untracked(), 0.75);
}
