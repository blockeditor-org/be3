use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[tokio::test]
async fn a_kept_answer_is_handed_out_until_its_executor_stops_answering() {
    let answering = Arc::new(AtomicBool::new(true));
    let probed = Arc::clone(&answering);
    let running = start(
        "kept",
        Some(Arc::new(move |_: &Path, text: &str| {
            assert!(text.contains("reapih.example"), "{text}");
            probed.load(Ordering::SeqCst)
        })),
    )
    .await;

    assert_eq!(running.get("GET").await.0, "HTTP/1.1 200 OK");
    assert_eq!(running.runs(), 1);

    running.age_by(Duration::from_secs(120)).await;
    let (status, age, _) = running.get("GET").await;
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(age.is_some_and(|age| age >= 120), "{age:?}");
    assert_eq!(running.runs(), 1);

    answering.store(false, Ordering::SeqCst);
    running.age_by(Duration::from_secs(120)).await;
    let (status, age, _) = running.get("GET").await;
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(age.is_some_and(|age| age < 60), "{age:?}");
    assert_eq!(running.runs(), 2);

    running.stop().await;
}
