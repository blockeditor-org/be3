use std::path::PathBuf;

use super::{options, run_ok, scratch, shell};

#[test]
fn the_test_gets_a_short_tmpdir_removed_after() {
    let directory = scratch("tmpdir");
    let out = directory.join("seen");
    let options = options(shell(&format!("touch \"$TMPDIR/file\" && printf %s \"$TMPDIR\" > '{}'", out.display())));
    assert_eq!(run_ok(&options), 0);
    let tmpdir = PathBuf::from(std::fs::read_to_string(&out).unwrap());
    assert!(tmpdir.to_string_lossy().starts_with("/tmp/test."), "{}", tmpdir.display());
    assert!(!tmpdir.exists());
}
