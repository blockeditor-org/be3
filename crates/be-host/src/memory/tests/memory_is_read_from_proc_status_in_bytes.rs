use super::*;

#[test]
fn memory_is_read_from_proc_status_in_bytes() {
    let status = "Name:\tbe-host\nVmPeak:\t 9999 kB\nVmRSS:\t  960512 kB\nRssAnon:\t  700000 kB\nRssFile:\t  160512 kB\nRssShmem:\t  100000 kB\nThreads:\t42\n";
    assert_eq!(
        from_status(status),
        MemoryUsage {
            resident: 960_512 * 1024,
            anonymous: 700_000 * 1024,
            file: 160_512 * 1024,
            shared: 100_000 * 1024,
            ..MemoryUsage::default()
        }
    );
}
