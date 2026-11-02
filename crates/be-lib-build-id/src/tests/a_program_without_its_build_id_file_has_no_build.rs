use super::*;

#[test]
fn a_program_without_its_build_id_file_has_no_build() {
    let error = current().unwrap_err();
    assert!(error.contains(FILE), "{error}");
}
