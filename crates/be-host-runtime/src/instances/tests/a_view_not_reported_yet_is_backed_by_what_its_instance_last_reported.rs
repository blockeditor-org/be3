use super::*;
use be_sdk_api::accessibility::Color;

const TAB: ViewId = ViewId(5);

fn report(instances: &mut Instances, view: ViewId, backdrop: Option<Color>) {
    let screen = instances
        .screen_id(INSTANCE, view)
        .expect("the view has a screen");
    instances.set_frame_reports(vec![be_sdk_api::FrameReport {
        screen,
        content: None,
        painted: Vec::new(),
        floating: Vec::new(),
        floating_painted: Vec::new(),
        marks: Vec::new(),
        marks_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        actions: None,
        anchored: None,
        work_area: None,
        backdrop,
    }]);
}

/// A dock tab's body is a view of its own, mounted again each time the tab is shown: until its
/// app reports a frame of it, the host fills it with the color the instance clears its other
/// views to, so a tab switched to never shows what is behind it.
#[test]
fn a_view_not_reported_yet_is_backed_by_what_its_instance_last_reported() {
    let mut instances = placed();
    let surface = Color {
        red: 22,
        green: 26,
        blue: 34,
        alpha: 255,
    };
    assert_eq!(instances.backdrop(INSTANCE), None, "nothing reported yet");
    report(&mut instances, ViewId::MAIN, Some(surface.clone()));

    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    let catalog = Arc::new(crate::AppCatalog::default());
    instances.mount(INSTANCE, View::dock(TAB), Uuid::nil(), role, &catalog);
    instances.report(
        INSTANCE,
        View::dock(TAB),
        Uuid::nil(),
        role,
        &catalog,
        None,
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    assert!(
        instances.frame_report(INSTANCE, TAB).is_none(),
        "the new view has no frame yet"
    );
    assert_eq!(
        instances.backdrop(INSTANCE),
        Some(&surface),
        "it is backed by what the instance's other view is cleared to"
    );

    report(&mut instances, TAB, None);
    assert_eq!(
        instances.backdrop(INSTANCE),
        Some(&surface),
        "a view that shows through reports none, which leaves the instance's color as it was"
    );
}
