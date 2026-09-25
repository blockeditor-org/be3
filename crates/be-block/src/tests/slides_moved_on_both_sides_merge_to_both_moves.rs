use super::*;
use crate::presentation::PresentationContent;

#[test]
#[ignore = "two reorders of one list conflict in the list merge and one side's move is dropped without counting a conflict"]
fn slides_moved_on_both_sides_merge_to_both_moves() {
    let (first, second, third) = (ObjectId::new(), ObjectId::new(), ObjectId::new());
    let mut base = PresentationContent::default();
    for (index, slide) in [first, second, third].into_iter().enumerate() {
        let edit = base.root().insert(slide, index, Uuid::new_v4());
        base.apply(&edit);
    }
    let ours = edited(&base, [base.root().move_to(first, 2)]);
    let theirs = edited(&base, [base.root().move_to(third, 0)]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    let order: Vec<ObjectId> = merged.root().slides.iter().map(|slide| slide.id).collect();
    assert_eq!(order, [third, second, first]);
}
