use super::*;

#[test]
fn import_file_slot_typed_operator_errors_in_unknown_slot() {
    let errors = import_file(
        "op.qxc",
        "#builtin.build .= () => std.Folder: [
  \"lib.c\" .= std.c.compile: [
    \"f\" .= f
  ]
]
f :: (a: std.c.int) => std.c.int: {
  x := a + 1
  -> x
}
std :: #builtin.std",
    )
    .expect_err("+ has no slot type to use");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].entries[0].message,
        "operator + is not supported in slot: TypeUnknown"
    );
}
