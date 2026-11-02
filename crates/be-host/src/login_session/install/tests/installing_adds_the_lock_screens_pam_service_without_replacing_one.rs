use super::*;

#[test]
fn installing_adds_the_lock_screens_pam_service_without_replacing_one() {
    let dir = std::env::temp_dir().join(format!("block-app-pam-{}", uuid::Uuid::new_v4()));
    assert_eq!(
        place_pam(&dir).unwrap(),
        None,
        "no PAM directory, no service"
    );

    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        crate::login_session::pam::service_in(&dir),
        crate::login_session::pam::FALLBACK_SERVICE,
        "without its own service the lock checks with login's"
    );
    let service = dir.join("block-app");
    assert_eq!(place_pam(&dir).unwrap(), Some(service.clone()));
    assert_eq!(
        crate::login_session::pam::service_in(&dir),
        crate::login_session::pam::SERVICE,
        "the service the installer places is the one the lock uses"
    );
    assert_eq!(
        std::fs::read_to_string(&service).unwrap(),
        "auth include login\n"
    );

    std::fs::remove_file(&service).unwrap();
    std::fs::write(dir.join("common-auth"), "auth required pam_unix.so\n").unwrap();
    place_pam(&dir).unwrap();
    assert_eq!(
        std::fs::read_to_string(&service).unwrap(),
        "auth include common-auth\n",
        "Debian and Ubuntu authenticate with common-auth, without login's delay and nologin"
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
