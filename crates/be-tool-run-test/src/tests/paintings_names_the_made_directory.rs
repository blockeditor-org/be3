use super::{options, run_ok, scratch, shell};

#[test]
fn paintings_names_the_made_directory() {
    let directory = scratch("paintings");
    let out = directory.join("paintings");
    let mut options = options(shell("touch \"$MADE_PAINTINGS/a.paint\""));
    options.paintings = Some(out.clone());
    assert_eq!(run_ok(&options), 0);
    assert!(out.join("a.paint").is_file());
}
