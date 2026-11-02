use super::*;

#[test]
fn pointer_lock() {
    drive("pointer_lock", include_str!("../drive/pointer_lock.drive"));
}
