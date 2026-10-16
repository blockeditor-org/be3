use super::*;

#[test]
fn remaps_run_on_a_device_only_once_it_turns_them_on() {
    let mut approval = Approval::default();
    approval.load(None);
    assert_eq!(approval.state(false), RemapState::Off);
    assert!(!approval.asking());

    assert!(approval.want(Some(vec![1])));
    assert!(approval.asking(), "new remaps ask before they run");
    assert_eq!(approval.running(), None);
    assert_eq!(approval.state(false), RemapState::Waiting);

    assert_eq!(approval.turn_on(), Some(vec![1]));
    assert!(!approval.asking());
    assert_eq!(approval.running(), Some(vec![1]));
    assert_eq!(approval.state(false), RemapState::On);
    assert_eq!(approval.state(true), RemapState::Stopped);

    assert!(approval.want(Some(vec![2])));
    assert_eq!(
        approval.running(),
        Some(vec![1]),
        "changed remaps keep the ones turned on running until the change is turned on"
    );
    assert!(approval.asking());

    assert!(approval.want(None));
    assert_eq!(approval.running(), None, "turning remaps off never asks");
    assert!(!approval.asking());

    let mut booted = Approval::default();
    booted.want(Some(vec![1]));
    booted.load(Some(vec![1]));
    assert!(
        !booted.asking(),
        "what this device turned on before runs at once"
    );
    assert_eq!(booted.running(), Some(vec![1]));
}
