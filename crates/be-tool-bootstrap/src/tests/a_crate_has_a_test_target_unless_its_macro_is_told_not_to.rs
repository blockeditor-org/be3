use crate::test::has_test_target;

#[test]
fn a_crate_has_a_test_target_unless_its_macro_is_told_not_to() {
    assert!(has_test_target("load(\"x\")\ncargo_crate()\n"));
    assert!(!has_test_target("cargo_crate(test = False)\n"));
    assert!(has_test_target("cargo_library()\n\ncargo_test(\n    paints = True,\n)\n"));
    assert!(has_test_target("type_module(module = \"m\")\n"));
    assert!(!has_test_target("cargo_library()\ncargo_binary()\n"));
}
