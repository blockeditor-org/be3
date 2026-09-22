use super::*;

#[test]
fn an_idle_pump_does_not_run_its_projections_again() {
    let client = client();
    let block = client.create_block(UiSettings::default());
    let source = BlockSource::new(block.clone(), || {});
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let zoom = source.project(move |settings| {
        counted.set(counted.get() + 1);
        settings.zoom()
    });

    assert_eq!(runs.get(), 1);
    for _ in 0..3 {
        source.pump();
    }
    assert_eq!(runs.get(), 1);
    assert_eq!(zoom.get_untracked(), 1.0);

    block.operate(UiSettingsOperation::SetZoom { zoom: 1.5 });
    block.operate(UiSettingsOperation::SetZoom { zoom: 2.0 });
    source.pump();
    assert_eq!(runs.get(), 2);
    assert_eq!(zoom.get_untracked(), 2.0);

    source.pump();
    assert_eq!(runs.get(), 2);
}
