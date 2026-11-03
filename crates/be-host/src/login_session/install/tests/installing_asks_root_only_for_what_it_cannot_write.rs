use super::*;

struct Fake {
    writable: Vec<PathBuf>,
    owners: Vec<(PathBuf, u32)>,
}

impl Access for Fake {
    fn writable(&self, path: &Path) -> bool {
        self.writable.iter().any(|dir| path.starts_with(dir))
    }

    fn owner(&self, path: &Path) -> Option<u32> {
        self.owners
            .iter()
            .filter(|(dir, _)| path.starts_with(dir))
            .max_by_key(|(dir, _)| dir.components().count())
            .map(|(_, owner)| *owner)
    }
}

#[test]
fn installing_asks_root_only_for_what_it_cannot_write() {
    let root = scratch("install-root");
    let source = root.join("app");
    build(&source);
    let home = root.join("home/qa");
    let entry = root.join("usr/share/wayland-sessions/be-host.desktop");
    let layout = Layout {
        prefix: home.join(".local"),
        sessions: vec![entry.parent().unwrap().to_path_buf()],
        pam: root.join("etc/pam.d"),
        greetd: root.join("etc/greetd/environments"),
    };
    let (changes, _) = plan_install(&source, &layout).unwrap();
    let user = User {
        root: false,
        home: Some(home.clone()),
    };
    let only = |writable: &[&Path]| Fake {
        writable: writable.iter().map(|dir| dir.to_path_buf()).collect(),
        owners: vec![(root.clone(), 0), (home.clone(), 1000)],
    };

    assert_eq!(
        changes::decide(&changes, &only(&[&root]), &user),
        Ok(Decision::Here),
        "what you can write, you install yourself"
    );
    assert_eq!(
        changes::decide(&changes, &only(&[&home]), &user),
        Ok(Decision::Elevate {
            whole: false,
            blocked: vec![entry.clone()],
        }),
        "a prefix in your home is yours, and only the entry needs root"
    );
    assert_eq!(
        changes::list(&[entry.clone()]),
        format!("Writing {}", entry.display()),
        "the path that needs root is named"
    );
    let refused = changes::decide(&changes, &only(&[]), &user).unwrap_err();
    assert!(
        refused.contains("in your home directory"),
        "nothing in your home is written as root: {refused}"
    );
    let rooted = User {
        root: true,
        home: None,
    };
    let refused = changes::decide(&changes, &only(&[]), &rooted).unwrap_err();
    assert!(
        refused.contains("belongs to the user with id 1000"),
        "root does not write into someone else's directory either: {refused}"
    );

    let system = Layout {
        prefix: root.join("usr/local"),
        ..layout
    };
    let (system_changes, _) = plan_install(&source, &system).unwrap();
    match changes::decide(&system_changes, &only(&[&home]), &user) {
        Ok(Decision::Elevate { whole: true, .. }) => {}
        decided => panic!("a system-wide prefix installs as root throughout: {decided:?}"),
    }
    assert_eq!(
        changes::decide(&system_changes, &only(&[]), &rooted),
        Ok(Decision::Here)
    );

    let installed = home.join(".local/lib/be-host");
    let link = home.join(".local/bin/be-host");
    let elevated: Vec<bool> = changes
        .iter()
        .map(|change| change.blocked(&only(&[&home])).is_some())
        .collect();
    let failed = changes::apply(&changes, &elevated, |_| Err("sudo had no terminal".into()));
    assert_eq!(failed.unwrap_err().to_string(), "sudo had no terminal");
    assert!(
        std::fs::read_dir(home.join(".local/lib"))
            .map(|mut dir| dir.next().is_none())
            .unwrap_or(true),
        "when root's part fails nothing is left, staged or installed"
    );
    assert!(std::fs::symlink_metadata(&link).is_err());
    assert!(!entry.exists());

    changes::apply(&changes, &elevated, |theirs| {
        assert_eq!(theirs.len(), 1, "sudo is handed the entry alone");
        let sent = changes::to_json(theirs)?;
        apply_here(&changes::from_json(&sent).unwrap())?;
        Ok(())
    })
    .unwrap();
    assert!(installed.join(EXECUTABLE).is_file());
    assert!(entry.is_file());
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        installed.join(EXECUTABLE)
    );
    std::fs::remove_dir_all(root).unwrap();
}
