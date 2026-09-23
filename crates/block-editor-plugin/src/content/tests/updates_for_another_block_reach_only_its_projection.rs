use super::*;

#[test]
fn updates_for_another_block_reach_only_its_projection() {
    let fixture = Fixture::new();
    let other_block = uuid::Uuid::new_v4();
    let other = ContentProjection::<ChecklistContent>::new(fixture.host.clone(), Some(other_block));
    let (_, add) = Checklist::add("from elsewhere");
    let mut content = ChecklistContent::default();
    content.apply(&add);
    fixture.host.set_content_of(
        other_block,
        ChecklistContent::CONTENT_TYPE,
        content.encode(),
        0,
    );

    other.pump();
    fixture.projection.pump();

    let listed = |projection: &ContentProjection<ChecklistContent>| {
        projection
            .read(|content| content.root().items.len())
            .unwrap_or_default()
    };
    assert_eq!(listed(&other), 1);
    assert_eq!(listed(&fixture.projection), 2);
    assert_eq!(fixture.runs(), [0, 0, 0]);

    let (_, second) = Checklist::add("second");
    other.operate(second);
    assert_eq!(
        fixture.host.take_content_operations_of(other_block).len(),
        1
    );
    assert!(fixture.host.take_content_operations().is_empty());
}
