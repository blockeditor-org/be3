use super::*;

#[test]
fn installing_adds_the_lock_screens_pam_service_without_replacing_one() {
    let dir = std::env::temp_dir().join(format!("block-app-pam-{}", uuid::Uuid::new_v4()));
    assert_eq!(place_pam(&dir).unwrap(), None, "no PAM directory, no service");

    std::fs::create_dir_all(&dir).unwrap();
    let service = dir.join("block-app");
    assert_eq!(place_pam(&dir).unwrap(), Some(service.clone()));
    assert_eq!(
        std::fs::read_to_string(&service).unwrap(),
        "auth include login\n"
    );

    std::fs::write(&service, "auth include system-auth\n").unwrap();
    assert_eq!(place_pam(&dir).unwrap(), None);
    assert_eq!(
        std::fs::read_to_string(&service).unwrap(),
        "auth include system-auth\n",
        "a service someone changed is kept"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
