use super::*;

#[test]
fn lightdms_configured_sessions_directory_gets_the_entry_too() {
    let root = std::env::temp_dir().join(format!("block-app-lightdm-{}", uuid::Uuid::new_v4()));
    let standard = root.join("usr/share/wayland-sessions");
    assert_eq!(
        session_dirs(&root),
        vec![standard.clone()],
        "without LightDM the entry goes where GDM, SDDM and LightDM all look"
    );

    std::fs::create_dir_all(root.join("usr/share/lightdm/lightdm.conf.d")).unwrap();
    std::fs::write(
        root.join("usr/share/lightdm/lightdm.conf.d/50-default.conf"),
        "[LightDM]\nsessions-directory=/usr/share/lightdm/sessions:/usr/share/xsessions:/usr/share/wayland-sessions\n",
    )
    .unwrap();
    assert_eq!(session_dirs(&root), vec![standard.clone()]);

    std::fs::create_dir_all(root.join("etc/lightdm")).unwrap();
    std::fs::write(
        root.join("etc/lightdm/lightdm.conf"),
        "[Seat:*]\nsessions-directory=/ignored\n[LightDM]\n# a comment\nsessions-directory = /opt/sessions:/usr/share/xsessions\n",
    )
    .unwrap();
    assert_eq!(
        session_dirs(&root),
        vec![standard, root.join("opt/sessions")],
        "a LightDM that doesn't read the standard directory gets the entry in its own"
    );
    std::fs::remove_dir_all(root).unwrap();
}
