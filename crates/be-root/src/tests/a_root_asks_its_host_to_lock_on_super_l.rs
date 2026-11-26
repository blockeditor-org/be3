use be_lib_ui_types::{Key, Modifiers};
use be_wasm_root_api::LockState;

use super::*;

fn lock(available: bool, locked: bool) -> LockState {
    LockState {
        generation: 0,
        available,
        locked,
        checking: false,
        refused: None,
        user: "Ada Lovelace".to_owned(),
        power: Vec::new(),
        retry_at: None,
    }
}

/// Hands the root Super+L and answers whether it asked its host to lock within a few frames.
fn super_l_asks(root: &mut Root, time: &mut u64) -> bool {
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::L, Modifiers::LOGO)),
    )
    .expect("the root takes input");
    (0..6).any(|_| {
        *time += 1;
        step(root, *time);
        root.take_lock_asked()
    })
}

#[test]
fn a_root_asks_its_host_to_lock_on_super_l() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    show_core(&mut root, be_wasm_root_api::CoreView::default());

    // A host that cannot lock is not asked to.
    root.show_lock(lock(false, false));
    hand(&mut root, &mut time, Vec::new());
    assert!(
        !super_l_asks(&mut root, &mut time),
        "Super+L asks nothing of a host that cannot lock"
    );

    // One that can is asked to by Super+L, wherever the keyboard is, as be-host's own UI locks
    // on it: be-root's menu has no Lock entry.
    root.show_lock(lock(true, false));
    hand(&mut root, &mut time, Vec::new());
    assert!(
        super_l_asks(&mut root, &mut time),
        "Super+L asks the host to lock the session"
    );

    // Once locked, the lock screen has the keyboard, and Super+L asks nothing more.
    root.show_lock(lock(true, true));
    hand(&mut root, &mut time, Vec::new());
    assert!(
        !super_l_asks(&mut root, &mut time),
        "Super+L asks nothing while the session is locked"
    );
}
