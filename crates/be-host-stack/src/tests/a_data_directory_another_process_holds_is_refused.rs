use super::*;

/// A process holds the data directory once it opens its refs database: the hosts of that
/// process share it, and another process opening it is refused, as two would each number edits
/// under the one origin this device keeps per block.
#[test]
fn a_data_directory_another_process_holds_is_refused() {
    let directory = std::env::temp_dir().join(format!("be-held-{}", Uuid::new_v4().simple()));
    let refs = crate::refs_in(&directory).expect("the refs database opens");
    let again = crate::refs_in(&directory).expect("this process opens it again");
    drop((refs, again));
    // As another process would: its own handle on the lock file.
    let error = crate::lock_data_dir(&directory).expect_err("another process is refused");
    assert!(
        error.starts_with("Another be-host is using this data directory"),
        "{error}"
    );
}
