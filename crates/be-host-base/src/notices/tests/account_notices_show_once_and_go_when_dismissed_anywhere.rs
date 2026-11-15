use super::*;

#[test]
fn account_notices_show_once_and_go_when_dismissed_anywhere() {
    let ours = |notices: &[Notice]| -> Vec<(u64, String)> {
        notices
            .iter()
            .filter_map(|notice| notice.account.map(|account| (account, notice.message.clone())))
            .filter(|(account, _)| *account >= 9001)
            .collect()
    };
    let added = (9001, "A passkey can now sign in to your account.".to_owned());
    let removed = (9002, "Google can no longer sign in to your account.".to_owned());
    account_notices(vec![added.clone(), removed.clone()]);
    // Read again, the same notices show once each.
    account_notices(vec![added.clone(), removed.clone()]);
    assert_eq!(ours(&shown()), vec![added.clone(), removed.clone()]);
    // Dismissed here, the caller is told which to dismiss on the server.
    let id = shown()
        .into_iter()
        .find(|notice| notice.account == Some(9001))
        .unwrap()
        .id;
    assert_eq!(dismiss(id), Some(9001));
    // Dismissed on another device, it goes from the list the server answers, and here too.
    account_notices(vec![]);
    assert!(ours(&shown()).is_empty());
}
