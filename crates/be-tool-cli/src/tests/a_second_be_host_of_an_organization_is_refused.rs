use super::*;

/// `be BLOCK host` holds its organization's data directory, as be-host does: a second one on
/// this machine, here a lock another process took, is refused with a reason before it starts.
#[test]
fn a_second_be_host_of_an_organization_is_refused() {
    let directory = std::env::temp_dir().join(format!("be-host-held-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&directory).expect("the directory is made");
    // As another process would: its own handle on the lock file.
    let other = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join("be-refs.lock"))
        .expect("the lock file opens");
    other.try_lock().expect("the other process holds it");
    let error = crate::session::hold_host_data(&directory).expect_err("a second one is refused");
    assert!(
        error.contains("another `be ... host` of this organization runs on this machine"),
        "{error}"
    );
    drop(other);
    let _ = std::fs::remove_dir_all(&directory);
}
