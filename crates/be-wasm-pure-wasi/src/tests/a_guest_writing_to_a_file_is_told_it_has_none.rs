use super::*;

#[test]
fn a_guest_writing_to_a_file_is_told_it_has_none() {
    let mut logged = Vec::new();
    let (errno, _) = write(&mut Wasi::new([0; 32]), 3, b"x\n", &mut |line| logged.push(line.to_owned()));
    assert_eq!(errno, EBADF);
    assert!(logged.is_empty());
}
