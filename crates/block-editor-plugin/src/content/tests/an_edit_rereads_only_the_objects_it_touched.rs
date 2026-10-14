use super::*;

#[test]
fn an_edit_rereads_only_the_objects_it_touched() {
    let fixture = Fixture::new();
    let items = fixture
        ._scope
        .run(|| fixture.projection.objects(ObjectId::ROOT, Checklist::ITEMS));
    let before = items.get_untracked();

    fixture.arrive(&[(Checklist::set_text(fixture.second, "oat milk"), false)]);
    let after = items.get_untracked();
    let held: Vec<_> = after.iter().map(|(id, _)| *id).collect();
    assert_eq!(held, [fixture.first, fixture.second]);
    let (first, second) = (after.iter().next().unwrap(), after.iter().nth(1).unwrap());
    assert!(Rc::ptr_eq(&first.1, &before.iter().next().unwrap().1));
    assert!(!Rc::ptr_eq(&second.1, &before.iter().nth(1).unwrap().1));
    assert_eq!(second.1.text, "oat milk");

    let (bread, added) = Checklist::add("bread");
    fixture.arrive(&[(added, false)]);
    let grown = items.get_untracked();
    assert_eq!(grown.len(), 3);
    assert_eq!(grown.iter().nth(2).unwrap().0, bread);
    assert!(Rc::ptr_eq(&grown.iter().next().unwrap().1, &first.1));
}
