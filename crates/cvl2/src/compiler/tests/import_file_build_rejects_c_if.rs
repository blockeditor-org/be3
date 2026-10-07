use super::*;

#[test]
fn import_file_build_rejects_c_if() {
    let errors = import_file(
        "target.qxc",
        "#builtin.build .= () => std.Folder: [
  \"x\" .= std.File: {
    std.c.if (std.c.int: 1) { }
    -> \"x\"
  }
]
std :: #builtin.std",
    )
    .expect_err("std.c.if runs at build time here");

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].entries[0].message,
        "std.c.if can't run at compile time"
    );
}
