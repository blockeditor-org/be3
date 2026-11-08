use super::{options, run_ok, scratch, shell};

#[test]
fn library_path_reaches_the_test() {
    let directory = scratch("library-path");
    let out = directory.join("seen");
    let mut options = options(shell(&format!(
        "printf %s \"$LD_LIBRARY_PATH\" > '{}'",
        out.display()
    )));
    options.library_path = Some("one:two".into());
    assert_eq!(run_ok(&options), 0);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "one:two");
}
