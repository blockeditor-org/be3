use block_type_api::ops::Outcome;

#[test]
fn a_modules_answer_is_seen_after_the_host_task_spent_its_budget() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a test runtime starts");
    runtime.block_on(async {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<()>();
        for _ in 0..1_000 {
            sender.send(()).expect("the receiver is held");
            let _ = futures_util::FutureExt::now_or_never(receiver.recv());
        }
        assert!(
            !tokio::task::coop::has_budget_remaining(),
            "the task spent its budget, as a host does that has just handled many messages"
        );
        let (answer, mut answered) = tokio::sync::oneshot::channel();
        answer
            .send(Outcome::returned(&()))
            .expect("the waiter is held");
        assert!(
            crate::replicas::answer_in(&mut answered).is_some(),
            "an answer already sent is taken, or the host waits on it forever"
        );
    });
}
