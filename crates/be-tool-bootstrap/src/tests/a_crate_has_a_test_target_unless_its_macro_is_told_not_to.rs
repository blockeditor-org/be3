use crate::test::test_targets;

#[test]
fn a_crate_has_a_test_target_unless_its_macro_is_told_not_to() {
    assert_eq!(test_targets("load(\"x\")\ncargo_crate()\n"), ["test"]);
    assert!(test_targets("cargo_crate(test = False)\n").is_empty());
    assert_eq!(
        test_targets("cargo_library()\n\ncargo_test(\n    paints = True,\n)\n"),
        ["test"]
    );
    assert_eq!(test_targets("type_module(module = \"m\")\n"), ["test"]);
    assert!(test_targets("cargo_library()\ncargo_binary()\n").is_empty());
    assert_eq!(
        test_targets("cargo_test(resources = glob([\"wit/*.wit\"]))\n"),
        ["test"]
    );
}
