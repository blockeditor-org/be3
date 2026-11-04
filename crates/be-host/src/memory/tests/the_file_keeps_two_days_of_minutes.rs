use super::*;

#[test]
fn the_file_keeps_two_days_of_minutes() {
    let directory = std::env::temp_dir().join(format!("be-host-memory-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("the directory is made");
    std::fs::write(directory.join(FILE), "the last run's\n").expect("a last run's file");
    let mut log = Log::open(&directory).expect("the file opens");
    assert_eq!(
        std::fs::read_to_string(directory.join(PREVIOUS))
            .ok()
            .as_deref(),
        Some("the last run's\n"),
        "the last run's file is kept"
    );
    let minute = |at: u64| Minute {
        point: MemoryPoint {
            at,
            usage: MemoryUsage {
                resident: 3 << 20,
                anonymous: 2 << 20,
                file: 1 << 20,
                wasm: 1 << 20,
                ..MemoryUsage::default()
            },
        },
        guests: [
            ("app be3:desktop".to_owned(), 512 << 10),
            ("app be3:text".to_owned(), 512 << 10),
        ]
        .into(),
    };
    assert_eq!(
        line(&minute(60)),
        "60\t3072\t2048\t1024\t0\t0\t0\t1024\tapp be3:desktop=512;app be3:text=512",
        "a line is the minute's start and its sizes in KiB"
    );
    for at in 0..2 * MINUTES as u64 + 5 {
        log.write(&minute(at * 60)).expect("the minute is written");
    }
    let written = std::fs::read_to_string(directory.join(FILE)).expect("the file is there");
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.first(), Some(&HEADER), "the header stays first");
    assert_eq!(
        lines.len(),
        1 + MINUTES + 5,
        "trimmed to two days when it reached four, then written on"
    );
    assert_eq!(
        lines.last().copied(),
        Some(line(&minute((2 * MINUTES as u64 + 4) * 60)).as_str())
    );
    let _ = std::fs::remove_dir_all(&directory);
}
