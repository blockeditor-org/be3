use super::*;
use crate::presentation::PresentationContent;

#[test]
#[ignore = "a move to an index anchors after the slide that was there, and lands in the middle when someone moved that slide"]
fn slides_moved_by_two_peers_at_once_both_move() {
    let (first, second, third) = (ObjectId::new(), ObjectId::new(), ObjectId::new());
    let mut base = PresentationContent::default();
    for (index, slide) in [first, second, third].into_iter().enumerate() {
        let edit = base.root().insert(slide, index, Uuid::new_v4());
        base.apply(&edit);
    }
    let to_end = base.root().move_to(first, 2);
    let to_start = base.root().move_to(third, 0);

    for sequenced in [
        edited(&base, [to_end.clone(), to_start.clone()]),
        edited(&base, [to_start, to_end]),
    ] {
        let order: Vec<ObjectId> = sequenced
            .root()
            .slides
            .iter()
            .map(|slide| slide.id)
            .collect();
        assert_eq!(order, [third, second, first]);
    }
}
