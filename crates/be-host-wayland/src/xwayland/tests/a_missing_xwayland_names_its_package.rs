use super::*;

#[test]
fn a_missing_xwayland_names_its_package() {
    let temp = std::env::temp_dir().join(format!("be-wayland-x11-missing-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("the test directory is made");
    let dir = temp.join(".X11-unix");

    let error = preflight(Some(temp.as_os_str()), &dir).expect_err("there is no Xwayland");
    assert!(matches!(error, StartError::NotInstalled));
    assert!(error.to_string().contains("xwayland package"));
    assert!(!dir.exists(), "nothing is made for an Xwayland that cannot run");
    assert!(preflight(None, &dir).is_err_and(|error| matches!(error, StartError::NotInstalled)));
    std::fs::remove_dir_all(&temp).expect("the test directory is removed");
}
