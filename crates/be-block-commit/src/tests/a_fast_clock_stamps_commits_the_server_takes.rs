use super::*;

/// A device whose clock runs hours fast stamps its commits by the server's clock it measured,
/// so the server, which refuses times past its own clock plus `MAX_COMMIT_AHEAD`, takes them; a
/// slow clock, or one not yet measured, stamps by itself.
#[test]
fn a_fast_clock_stamps_commits_the_server_takes() {
    let server = 1_000 * HOUR;
    let fast = server + 3 * HOUR;
    let stamped = commit_time(fast, Some(server - fast));
    assert!(stamped <= server + MAX_COMMIT_AHEAD, "{stamped}");
    assert!(stamped >= server, "{stamped}");

    let slow = server - HOUR;
    assert_eq!(commit_time(slow, Some(server - slow)), slow);
    assert_eq!(commit_time(fast, None), fast);
}
