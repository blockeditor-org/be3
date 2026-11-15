use crate::replies::{Reply, expect};

/// A reply crosses from the page to the worker as bytes: the copy that encoded it no longer
/// stands for the waiting, and the decoded one answers it. A reply dropped unanswered drops
/// what waits on it, as dropping a callback would.
#[test]
fn a_reply_travels_as_its_number_and_one_dropped_unanswered_drops_what_waits() {
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
    assert!(
        hearing.recv().is_err(),
        "what waited is dropped without an answer"
    );
}
