use super::*;
use crate::paint::{PaintReview, PaintReviewContent};
use crate::{ChildChange, Root};
use uuid::Uuid;

#[test]
fn a_review_forgets_or_repoints_an_approval_with_its_snapshot() {
    let (button, dialog, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let review = edited(
        &PaintReviewContent::default(),
        [
            PaintReview::approve("ui/button.paint", "b1", button),
            PaintReview::approve("ui/dialog.paint", "d1", dialog),
        ],
    );
    assert_eq!(BlockContent::references(&review), [button, dialog]);

    let changes = [
        ChildChange::Delete(button),
        ChildChange::Replace {
            old: dialog,
            new: copy,
        },
    ];
    let review = edited(
        &review,
        changes.map(|change| review.root().child_edit(change).expect("a review follows")),
    );

    let approved = review.root().approved();
    assert_eq!(approved.len(), 1);
    assert_eq!(approved[0].path, "ui/dialog.paint");
    assert_eq!(approved[0].hash, "d1");
    assert_eq!(approved[0].snapshot, copy);
}
