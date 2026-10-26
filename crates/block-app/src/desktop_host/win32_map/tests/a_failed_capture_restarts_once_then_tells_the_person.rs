use super::*;

#[test]
fn a_failed_capture_restarts_once_then_tells_the_person() {
    assert_eq!(recovery(None, false, "Paint"), Recovery::Stop);
    assert_eq!(recovery(Some("lost"), false, "Paint").notice(), None);
    assert_eq!(recovery(Some("lost"), false, "Paint"), Recovery::Restart);
    assert_eq!(
        recovery(Some("lost"), true, "Paint"),
        Recovery::Notice("Paint stopped being drawn, since capturing it failed twice: lost".to_owned())
    );
    assert_eq!(
        recovery(Some("lost"), true, ""),
        Recovery::Notice(
            "A window stopped being drawn, since capturing it failed twice: lost".to_owned()
        )
    );
}
