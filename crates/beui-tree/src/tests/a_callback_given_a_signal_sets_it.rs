use super::*;
use crate::reactive::{Callback, IntoCallback};

#[test]
fn a_callback_given_a_signal_sets_it() {
    let _root = ::reactive::RootScope::new();
    let picked = create_signal(0_usize);
    let callback: Callback<usize> = picked.into_callback();
    assert!(!callback.is_empty());
    callback.call(3);
    assert_eq!(picked.get_untracked(), 3);
}
