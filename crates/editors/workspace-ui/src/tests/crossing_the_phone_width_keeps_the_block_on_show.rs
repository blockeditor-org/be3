use block_editor_beui::TopBar;

use super::*;

fn placement(fixture: &Fixture, id: Uuid) -> block_editor_beui::ChildPlacement {
    fixture
        .test
        .children()
        .iter()
        .find(|placement| Uuid::from_bytes(placement.block_id) == id)
        .copied()
        .expect("the block is on show")
}

#[test]
fn crossing_the_phone_width_keeps_the_block_on_show() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    show(&mut fixture, opened, None);
    let phone = placement(&fixture, opened);
    assert_eq!(phone.top_bar, TopBar::Phone { open_files: 1 });

    fixture.test.set_size(Vec2::new(1200.0, 800.0));
    fixture.settle();
    let desktop = placement(&fixture, opened);
    assert_eq!(
        desktop.child, phone.child,
        "widening the window keeps the child the phone was showing"
    );
    assert_eq!(desktop.top_bar, TopBar::Shown);
    assert!(
        desktop.rect.x > 0.0,
        "on a desktop the block sits beside the files"
    );

    fixture.test.set_size(Vec2::new(390.0, 800.0));
    fixture.settle();
    let narrowed = placement(&fixture, opened);
    assert_eq!(narrowed.child, phone.child);
    assert_eq!(narrowed.top_bar, TopBar::Phone { open_files: 1 });
}
