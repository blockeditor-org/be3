use super::*;

#[test]
fn stale_sockets_of_ended_runs_are_swept_and_nothing_else() {
    let temp = std::env::temp_dir().join(format!("be-wayland-sweep-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("the test directory is made");
    let ended = 4_194_305;
    let ours = std::process::id();
    let parent = std::os::unix::process::parent_id();
    for name in [
        format!("be-wayland-{ended}"),
        format!("be-wayland-{ended}.lock"),
        format!("be-wayland-{parent}"),
        format!("be-wayland-{parent}.lock"),
        format!("be-wayland-{ours}.lock"),
        "be-wayland-gtk-test-1".to_owned(),
        format!("be-wayland-{ended}x"),
        format!("other-{ended}"),
    ] {
        std::fs::write(temp.join(name), b"").expect("a file is made");
    }
    let ended_dir = temp.join(format!("be-wayland-{}", ended - 1));
    std::fs::create_dir(&ended_dir).expect("a directory is made");
    std::fs::write(ended_dir.join("wayland-0"), b"").expect("a file in it is made");

    sweep(&temp);

    let mut left: Vec<String> = std::fs::read_dir(&temp)
        .expect("the test directory is read")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .into_string()
                .expect("a name")
        })
        .collect();
    left.sort();
    let mut expected = vec![
        format!("be-wayland-{parent}"),
        format!("be-wayland-{parent}.lock"),
        format!("be-wayland-{ours}.lock"),
        "be-wayland-gtk-test-1".to_owned(),
        format!("be-wayland-{ended}x"),
        format!("other-{ended}"),
    ];
    expected.sort();
    assert_eq!(
        left, expected,
        "only the sockets, locks and directories of processes that have ended are removed"
    );

    let runtime = RuntimeDir::private_in(&temp).expect("a private runtime directory is made");
    let path = runtime.path().to_owned();
    let metadata = path.symlink_metadata().expect("it exists");
    assert!(
        metadata.is_dir() && metadata.mode() & 0o077 == 0,
        "it is ours alone"
    );
    assert!(runtime.private());
    drop(runtime);
    assert!(!path.exists(), "it goes with the server that made it");
    let _ = std::fs::remove_dir_all(&temp);
}
