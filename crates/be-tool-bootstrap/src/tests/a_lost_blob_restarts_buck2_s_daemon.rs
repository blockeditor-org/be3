use super::*;

/// A stand-in buck2 in `directory` that records each run in `calls`. Until it has been killed, a build appends what
/// re-relay logs for a refused `Execute` to `re-relay.log` if `forgets`, and exits 3; once killed, it exits 0.
#[cfg(unix)]
fn stand_in(directory: &std::path::Path, forgets: bool) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = std::fs::remove_dir_all(directory);
    std::fs::create_dir_all(directory).unwrap();
    let forget = if forgets {
        "echo 're-relay: 2026-11-26T10:00:00Z forgot ab12/40: the server refused an Execute' >> \"$DIR/re-relay.log\""
    } else {
        "echo 're-relay: 2026-11-26T10:00:00Z broke off Execute, making it again' >> \"$DIR/re-relay.log\""
    };
    let script = format!(
        "#!/bin/sh\nDIR='{}'\necho \"$*\" >> \"$DIR/calls\"\nif [ \"$1\" = kill ]; then touch \"$DIR/killed\"; exit 0; fi\n\
         if [ -e \"$DIR/killed\" ]; then exit 0; fi\n{forget}\nexit 3\n",
        directory.display()
    );
    let buck2 = directory.join("buck2");
    std::fs::write(&buck2, script).unwrap();
    std::fs::set_permissions(&buck2, std::fs::Permissions::from_mode(0o755)).unwrap();
    buck2
}

#[cfg(unix)]
fn drive(directory: &std::path::Path, buck2: &std::path::Path) -> (i32, Vec<String>) {
    let arguments = ["build".to_owned(), "//a:b".to_owned()];
    let mut configured = 0;
    let (status, _) = buck::retrying_with(
        buck2,
        &arguments,
        3,
        &directory.join("re-relay.log"),
        |arguments| {
            let status = std::process::Command::new(buck2)
                .args(arguments)
                .status()
                .unwrap();
            Ok((status.code().unwrap(), Vec::new()))
        },
        || {
            configured += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        configured, 0,
        "only an infrastructure error configures again"
    );
    let calls = std::fs::read_to_string(directory.join("calls")).unwrap();
    (status, calls.lines().map(str::to_owned).collect())
}

#[cfg(unix)]
#[test]
fn a_lost_blob_restarts_buck2_s_daemon() {
    let root = std::env::temp_dir().join(format!("be3-lost-blob-{}", std::process::id()));

    let lost = root.join("lost");
    let buck2 = stand_in(&lost, true);
    let (status, calls) = drive(&lost, &buck2);
    assert_eq!(status, 0);
    assert_eq!(calls, ["build //a:b", "kill", "build //a:b"]);

    // An action that failed: re-relay logged something else during the run, and a `forgot` only before it.
    let failed = root.join("failed");
    let buck2 = stand_in(&failed, false);
    std::fs::write(
        failed.join("re-relay.log"),
        "re-relay: 2026-11-26T09:00:00Z forgot cd34/40: the server refused an Execute\n",
    )
    .unwrap();
    let (status, calls) = drive(&failed, &buck2);
    assert_eq!(status, 3);
    assert_eq!(calls, ["build //a:b"]);

    let _ = std::fs::remove_dir_all(&root);
}
