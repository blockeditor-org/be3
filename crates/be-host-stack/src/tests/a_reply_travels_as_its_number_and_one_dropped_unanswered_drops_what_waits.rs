use super::*;
use crate::replies::{Reply, expect};

/// A reply crosses from the page to the worker as bytes: the copy that encoded it no longer
/// stands for the waiting, and the decoded one answers it. What still waits on a reply when the
/// stack stops is dropped without an answer, as dropping a callback would; and with no stack,
/// what would wait is dropped at once, as its command is.
#[test]
fn a_reply_travels_as_its_number_and_one_dropped_unanswered_drops_what_waits() {
    let harness = Harness::start();
    harness.connect();
    let (heard, hearing) = std::sync::mpsc::channel();
    let reply = expect(move |answer: Result<usize, String>| {
        let _ = heard.send(answer);
    });
    let bytes = postcard::to_stdvec(&reply).expect("it encodes");
    drop(reply);
    let decoded: Reply<Result<usize, String>> = postcard::from_bytes(&bytes).expect("it decodes");
    decoded.send(Ok(3));
    assert_eq!(hearing.recv(), Ok(Ok(3)));

    let (heard, hearing) = std::sync::mpsc::channel::<()>();
    let reply = expect(move |(): ()| {
        let _ = heard.send(());
    });
    let decoded: Reply<()> =
        postcard::from_bytes(&postcard::to_stdvec(&reply).expect("it encodes")).expect("decodes");
    drop(reply);
    drop(decoded);
    stop();
    assert!(
        hearing.recv().is_err(),
        "what waited is dropped without an answer"
    );

    let (heard, hearing) = std::sync::mpsc::channel::<()>();
    drop(expect(move |(): ()| {
        let _ = heard.send(());
    }));
    assert!(hearing.recv().is_err(), "with no stack nothing waits");
}
