use super::*;

use block_plugin_api::{Idle, IdleState, LockState, ScreenLock, UnlockAttempt};

#[test]
fn only_the_shell_sees_the_lock_and_may_send_it_a_password() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let locked = LockState {
        locked: true,
        user: "Ada Lovelace".to_owned(),
        ..LockState::default()
    };
    let attempt = UnlockAttempt {
        password: "hunter2".to_owned(),
    };

    assert!(watch::<ScreenLock>(&mut instances));
    assert!(watch::<Idle>(&mut instances));
    assert!(
        !publish::<ScreenLock>(&mut instances, &locked),
        "an editor that is not the shell is not told about the lock"
    );
    assert!(!publish::<Idle>(&mut instances, &IdleState { lock_due: true }));
    assert!(host_values_sent::<ScreenLock>(&instances.next_screens(PASS).opened).is_empty());
    assert!(
        !act(&mut instances, &attempt),
        "nor may it send a password to be checked"
    );
    assert!(take_actions::<UnlockAttempt>(&mut instances).is_empty());

    assert!(instances.set_shell(Some(INSTANCE)));
    assert_eq!(
        host_values_sent::<ScreenLock>(&instances.next_screens(PASS).opened),
        vec![locked]
    );
    assert!(act(&mut instances, &attempt));
    assert_eq!(take_actions::<UnlockAttempt>(&mut instances), vec![attempt]);
}
