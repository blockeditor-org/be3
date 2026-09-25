use super::*;

#[test]
fn a_picker_opened_from_a_creation_dialog_is_shown_above_it() {
    let outer = Uuid::new_v4();
    let inner = Uuid::new_v4();

    publish(creating(inner, 1));
    publish(creating(outer, 0));

    let shown: Vec<Uuid> = views().into_iter().map(|view| view.id).collect();
    assert_eq!(shown, [outer, inner]);
    assert_eq!(creation_surface(0), SurfaceId::Creation);
    assert_eq!(creation_surface(1), SurfaceId::NestedCreation);
    assert!(views().is_empty());
}
