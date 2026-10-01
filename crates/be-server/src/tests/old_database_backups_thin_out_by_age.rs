use super::*;

use crate::backup::{self, Directory, Target};

#[test]
fn old_database_backups_thin_out_by_age() {
    let target = Directory(std::env::temp_dir().join(format!("be-backup-test-{}", Uuid::new_v4())));
    let hour = 60 * 60;
    let now = 1_800_000_000 - 1_800_000_000 % (24 * hour);
    let mut taken = Vec::new();
    for hours_ago in (0..24 * 400).step_by(6) {
        let name = format!("{}.sealed", backup::timestamp(now - hours_ago * hour));
        target.put(&format!("databases/{name}"), b"sealed").unwrap();
        taken.push((hours_ago, name));
    }
    backup::prune(&target, now).unwrap();
    let kept: Vec<String> = target.list("databases").unwrap();
    let kept_hours: Vec<u64> = taken
        .iter()
        .filter(|(_, name)| kept.contains(name))
        .map(|(hours_ago, _)| *hours_ago)
        .collect();

    assert!(
        kept_hours.iter().filter(|hours| **hours < 48).count() == 8,
        "every backup of the last two days stays: {kept_hours:?}"
    );
    let daily = kept_hours
        .iter()
        .filter(|hours| (48..24 * 30).contains(*hours))
        .count();
    assert!((27..=29).contains(&daily), "one a day for a month: {daily}");
    let monthly = kept_hours
        .iter()
        .filter(|hours| (24 * 30..24 * 365).contains(*hours))
        .count();
    assert!(
        (10..=12).contains(&monthly),
        "one a month for a year: {monthly}"
    );
    assert!(
        kept_hours.iter().all(|hours| *hours < 24 * 365),
        "a backup older than a year stayed"
    );
    let _ = std::fs::remove_dir_all(&target.0);
}
