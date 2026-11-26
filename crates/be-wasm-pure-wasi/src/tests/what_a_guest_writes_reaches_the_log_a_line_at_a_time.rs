use super::*;

#[test]
fn what_a_guest_writes_reaches_the_log_a_line_at_a_time() {
    let mut wasi = Wasi::new([0; 32]);
    let mut logged = Vec::new();
    let mut log = |line: &str| logged.push(line.to_owned());
    assert_eq!(write(&mut wasi, 2, b"hello\nwor", &mut log), (SUCCESS, 9));
    assert_eq!(write(&mut wasi, 1, b"ld\n", &mut log), (SUCCESS, 3));
    assert_eq!(logged, ["hello", "world"]);
}
