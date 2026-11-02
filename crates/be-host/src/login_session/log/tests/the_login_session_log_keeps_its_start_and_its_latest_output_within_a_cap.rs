use super::*;

#[test]
fn the_login_session_log_keeps_its_start_and_its_latest_output_within_a_cap() {
    let directory = std::env::temp_dir().join(format!("be-host-log-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let cap = Cap {
        limit: 100,
        head: 30,
        parts: 2,
    };
    let path = log_path(&directory, LOG, 0);
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    let read = |part: usize| std::fs::read_to_string(log_path(&directory, LOG, part)).unwrap();

    write!(log, "Block started.\nthe start\n").unwrap();
    assert!(
        !cap.check(&mut log, &directory).unwrap(),
        "under the cap it stays"
    );
    let first: String = (0..10).map(|line| format!("first {line}\n")).collect();
    log.write_all(first.as_bytes()).unwrap();
    assert!(cap.check(&mut log, &directory).unwrap());
    assert!(
        read(0).starts_with("Block started.\nthe start\n"),
        "the start is kept, cut at a line: {}",
        read(0)
    );
    assert!(
        read(0).contains("is in"),
        "the log says where the rest went"
    );
    assert_eq!(
        read(1),
        first,
        "what followed the start moves to the first part"
    );

    let second: String = (0..10).map(|line| format!("second {line}\n")).collect();
    log.write_all(second.as_bytes()).unwrap();
    cap.check(&mut log, &directory).unwrap();
    assert!(read(1).ends_with(&second), "the newest part is the first");
    assert_eq!(read(2), first, "older parts move up");
    let third: String = (0..10).map(|line| format!("third {line}\n")).collect();
    log.write_all(third.as_bytes()).unwrap();
    cap.check(&mut log, &directory).unwrap();
    assert!(read(1).ends_with(&third));
    assert!(read(2).ends_with(&second));
    assert!(
        !log_path(&directory, LOG, 3).exists(),
        "only so many parts are kept"
    );
    write!(log, "a panic\n").unwrap();
    assert!(
        read(0).starts_with("Block started.\nthe start\n") && read(0).ends_with("a panic\n"),
        "output after a cut lands at the end of the log: {}",
        read(0)
    );

    keep_previous(&directory, cap);
    assert!(!path.exists());
    assert!(read_previous(&directory, 0).ends_with("a panic\n"));
    assert!(read_previous(&directory, 2).ends_with(&second));
    std::fs::write(&path, "the next session\n").unwrap();
    keep_previous(&directory, cap);
    assert_eq!(read_previous(&directory, 0), "the next session\n");
    assert!(
        !log_path(&directory, PREVIOUS, 1).exists(),
        "a session before the previous one leaves nothing"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

fn read_previous(directory: &Path, part: usize) -> String {
    std::fs::read_to_string(log_path(directory, PREVIOUS, part)).unwrap()
}
