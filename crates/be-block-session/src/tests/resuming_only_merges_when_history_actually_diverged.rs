use super::*;

#[test]
fn resuming_only_merges_when_history_actually_diverged() {
    let commits = commits();
    let base = write(&commits, "shared\n", 1_000, None);
    let ahead = write(&commits, "shared\nremote\n", 2_000, Some(base));
    let ours = write(&commits, "shared\nlocal\n", 2_100, Some(base));

    assert_eq!(resume(&commits, None, None).unwrap(), Resume::Empty);
    assert_eq!(
        resume(&commits, None, Some(base)).unwrap(),
        Resume::FastForward { to: base }
    );
    assert_eq!(
        resume(&commits, Some(base), None).unwrap(),
        Resume::Publish { from: base }
    );
    assert_eq!(
        resume(&commits, Some(base), Some(base)).unwrap(),
        Resume::UpToDate
    );
    assert_eq!(
        resume(&commits, Some(base), Some(ahead)).unwrap(),
        Resume::FastForward { to: ahead }
    );
    assert_eq!(
        resume(&commits, Some(ahead), Some(base)).unwrap(),
        Resume::Publish { from: ahead }
    );
    assert_eq!(
        resume(&commits, Some(ours), Some(ahead)).unwrap(),
        Resume::Merge {
            base: Some(base),
            ours,
            theirs: ahead,
        }
    );

    let reconciled = merged(
        &commits,
        "shared\nlocal\nremote\n",
        3_000,
        vec![ours, ahead],
    );
    assert_eq!(
        resume(&commits, Some(reconciled), Some(ahead)).unwrap(),
        Resume::Publish { from: reconciled }
    );
}
