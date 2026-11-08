use super::{options, run_ok, scratch, shell};

#[test]
fn paintings_names_used_and_changed() {
    let directory = scratch("paintings");
    let out = directory.join("paintings");
    let mut options = options(shell(
        "touch \"$USED_PAINTINGS/a\" \"$CHANGED_PAINTINGS/b\"",
    ));
    options.paintings = Some(out.clone());
    assert_eq!(run_ok(&options), 0);
    assert!(out.join("used/a").is_file());
    assert!(out.join("changed/b").is_file());
}
