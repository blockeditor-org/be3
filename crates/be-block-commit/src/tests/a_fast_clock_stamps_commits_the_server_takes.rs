use super::*;

/// A device whose clock runs hours fast stamps its commits by the server's clock it measured,
/// so the server, which refuses times past its own clock plus `MAX_COMMIT_AHEAD`, takes them; a
/// slow clock, or one not yet measured, stamps by itself. A stamp never goes below the commit it
/// follows while this clock has passed it, and a measurement that would pull stamps back by more
/// than a day is not trusted.
#[test]
fn a_fast_clock_stamps_commits_the_server_takes() {
    let server = 1_000 * HOUR;
    let fast = server + 3 * HOUR;
    let stamped = commit_time(fast, Some(server), None);
    assert!(stamped <= server + MAX_COMMIT_AHEAD, "{stamped}");
    assert!(stamped >= server, "{stamped}");

    let slow = server - HOUR;
    assert_eq!(commit_time(slow, Some(server), None), slow);
    assert_eq!(commit_time(fast, None, None), fast);

    let parent = server + 2 * HOUR;
    assert_eq!(commit_time(fast, Some(server), Some(parent)), parent);
    assert_eq!(commit_time(slow, Some(server), Some(parent)), slow);

    assert!(trusts_measurement(fast, server));
    assert!(!trusts_measurement(server + 2 * DAY, server));
}
