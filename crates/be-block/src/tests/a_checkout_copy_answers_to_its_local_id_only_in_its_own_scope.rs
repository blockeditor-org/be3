use super::*;
use crate::{
    BlockMetadata,
    version_control::{local_id, masked, scope_mask},
};

#[test]
fn a_checkout_copy_answers_to_its_local_id_only_in_its_own_scope() {
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    let local = Uuid::new_v4();
    let mask = scope_mask(first);
    let copy = masked(local, mask);
    let tagged = BlockMetadata {
        local_id: Some(local),
        ..BlockMetadata::default()
    };

    assert_ne!(mask, 0);
    assert_eq!(mask, scope_mask(first));
    assert_ne!(mask, scope_mask(second));
    assert_ne!(copy, local);
    assert_eq!(masked(copy, mask), local);
    assert_eq!(local_id(copy, &tagged, mask), local);
    assert_eq!(
        local_id(copy, &tagged, scope_mask(second)),
        copy,
        "another checkout's copy kept its own id"
    );
    assert_eq!(
        local_id(local, &BlockMetadata::default(), mask),
        local,
        "an adopted block kept its id"
    );
}
