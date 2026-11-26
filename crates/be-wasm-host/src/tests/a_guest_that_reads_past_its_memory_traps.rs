use super::*;

/// With V8's trap handler on, an access past a guest's memory reaches the guard regions around it
/// and is caught by the handler (engine::memories_stay_put), which must turn it into a trap
/// rather than end the process.
#[test]
fn a_guest_that_reads_past_its_memory_traps() {
    let source = guest("(drop (i32.load (i32.const 1000000)))");
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    app.start().unwrap();

    let failure = app.step().unwrap_err();

    assert!(
        failure.contains("memory access out of bounds"),
        "it says why it stopped: {failure}"
    );
}
