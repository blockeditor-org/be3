use super::*;

#[test]
fn uninstalling_removes_exactly_what_installing_placed() {
    let root = scratch("uninstall");
    let source = root.join("app");
    let layout = layout(&root);
    build(&source);
    std::fs::create_dir_all(&layout.pam).unwrap();
    std::fs::create_dir_all(layout.greetd.parent().unwrap()).unwrap();
    std::fs::write(&layout.greetd, "sway\n").unwrap();
    let others = layout.sessions[0].join("sway.desktop");
    std::fs::create_dir_all(&layout.sessions[0]).unwrap();
    std::fs::write(&others, "[Desktop Entry]\n").unwrap();
    let bin = layout.prefix.join("bin/other");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, "another program").unwrap();

    let receipt = install_into(&source, &layout);
    assert!(receipt.pam.is_some() && receipt.greetd.is_some());
    let home = layout.home();
    let kept = home.join("data/notes/mine.txt");
    std::fs::write(&kept, "a file the install did not put there").unwrap();

    apply_here(&plan_uninstall(&layout).unwrap()).unwrap();
    for gone in receipt.entries.iter().chain(&receipt.portal).chain([
        &receipt.link,
        &receipt.pam.as_ref().unwrap().0,
        &home.join(EXECUTABLE),
        &home.join(RECEIPT),
        &home.join(LIST),
        &home.join("data/notes/files"),
    ]) {
        assert!(
            std::fs::symlink_metadata(gone).is_err(),
            "{} is gone",
            gone.display()
        );
    }
    assert_eq!(
        std::fs::read_to_string(&layout.greetd).unwrap(),
        "sway\n",
        "greetd's list loses only its line"
    );
    assert!(others.is_file(), "other sessions' entries stay");
    assert!(
        layout.portals.is_dir(),
        "xdg-desktop-portal's directory stays, only be-host's files go"
    );
    assert!(bin.is_file(), "other programs in the prefix stay");
    assert!(kept.is_file(), "a file the install did not put there stays");

    std::fs::remove_dir_all(&home).unwrap();
    install_into(&source, &layout);
    std::fs::write(
        layout.pam.join(crate::login_session::pam::SERVICE),
        "auth include system-auth\n",
    )
    .unwrap();
    apply_here(&plan_uninstall(&layout).unwrap()).unwrap();
    assert!(!home.exists(), "an install alone leaves nothing behind");
    assert!(
        layout
            .pam
            .join(crate::login_session::pam::SERVICE)
            .is_file(),
        "a PAM service someone changed since stays"
    );
    assert!(
        plan_uninstall(&layout)
            .unwrap_err()
            .contains("nothing to uninstall")
    );
    std::fs::remove_dir_all(root).unwrap();
}
