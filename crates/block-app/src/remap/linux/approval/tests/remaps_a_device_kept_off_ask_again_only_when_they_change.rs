use super::*;

#[test]
fn remaps_a_device_kept_off_ask_again_only_when_they_change() {
    let mut approval = Approval::default();
    approval.want(Some(vec![1]));
    approval.keep_off();
    assert!(!approval.asking());
    assert_eq!(approval.state(false), RemapState::Waiting);

    approval.want(None);
    approval.want(Some(vec![1]));
    assert!(!approval.asking(), "the same remaps do not ask twice");

    approval.review();
    assert!(
        approval.asking(),
        "until the input settings ask to review them"
    );
    approval.keep_off();

    approval.want(Some(vec![2]));
    assert!(approval.asking(), "different remaps ask again");
}
