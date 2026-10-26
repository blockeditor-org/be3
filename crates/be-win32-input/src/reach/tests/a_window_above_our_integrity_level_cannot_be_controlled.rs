use super::*;

#[test]
fn a_window_above_our_integrity_level_cannot_be_controlled() {
    assert_eq!(reachable(INTEGRITY_MEDIUM, Some(INTEGRITY_MEDIUM)), Ok(()));
    assert_eq!(reachable(INTEGRITY_MEDIUM, Some(INTEGRITY_LOW)), Ok(()));
    assert_eq!(
        reachable(INTEGRITY_MEDIUM, Some(INTEGRITY_HIGH)),
        Err(InjectError::Elevated)
    );
    assert_eq!(
        reachable(INTEGRITY_MEDIUM, Some(INTEGRITY_SYSTEM)),
        Err(InjectError::Elevated)
    );
    assert_eq!(reachable(INTEGRITY_HIGH, Some(INTEGRITY_HIGH)), Ok(()));
    assert_eq!(
        reachable(INTEGRITY_MEDIUM, None),
        Err(InjectError::Unreachable)
    );
    assert!(InjectError::Elevated.needs_administrator());
    assert!(InjectError::Unreachable.needs_administrator());
    assert!(!InjectError::SecureDesktop.needs_administrator());
}
