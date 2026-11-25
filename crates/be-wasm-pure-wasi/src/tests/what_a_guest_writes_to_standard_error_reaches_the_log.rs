use super::*;

#[test]
fn what_a_guest_writes_to_standard_error_reaches_the_log() {
    // Two iovecs at 0, "he" at 16 and "llo" at 18, the count written at 32.
    let mut memory = Bytes(vec![0; 40]);
    for (at, word) in [(0, 16u32), (4, 2), (8, 18), (12, 3)] {
        memory.write(at, &word.to_le_bytes()).unwrap();
    }
    memory.write(16, b"hello").unwrap();
    let mut wasi = Wasi::new([0; 32]);
    let mut logged = Vec::new();
    let answered = wasi.call("fd_write", &[2, 0, 2, 32], &mut memory, &mut |bytes| {
        logged.extend_from_slice(bytes)
    });
    assert_eq!(answered, Ok(SUCCESS));
    assert_eq!(logged, b"hello");
    assert_eq!(memory.read(32, 4).unwrap(), 5u32.to_le_bytes());
    let to_a_file = wasi.call("fd_write", &[3, 0, 2, 32], &mut memory, &mut |_| {});
    assert_eq!(to_a_file, Ok(EBADF));
    let mut sizes = Bytes(vec![0xff; 8]);
    let answered = wasi.call("environ_sizes_get", &[0, 4], &mut sizes, &mut |_| {});
    assert_eq!((answered, sizes.0), (Ok(SUCCESS), vec![0; 8]));
    let clock = wasi.call("clock_time_get", &[1, 0, 0], &mut memory, &mut |_| {});
    assert_eq!(clock, Ok(ENOSYS));
}
