use super::*;

#[test]
fn a_pure_guest_may_not_import_more_of_wasi_than_it_is_answered() {
    let refused = Host::pure()
        .unwrap()
        .load_bytes(include_str!("reads_a_file.wat").as_bytes())
        .err()
        .expect("a guest that reads a file is refused");
    assert!(refused.contains("fd_read"), "{refused}");
}
