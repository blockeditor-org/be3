use std::time::Duration;

use super::*;

#[tokio::test]
async fn the_watcher_syncs_both_ways() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let note = text_block(&peer, code, "note.md", "first\n").await;
    let checkout = harness.checkout("checkout");
    let (passed, mut passes) = tokio::sync::mpsc::unbounded_channel();
    let (stop, stopped) = oneshot::channel::<()>();
    let target = Target { root: Some(code) };
    let mut types = types().await;
    let watching = crate::watch::watch(
        &peer,
        &mut types,
        &checkout,
        &target,
        Duration::from_millis(50),
        async {
            let _ = stopped.await;
        },
        move |report| {
            let _ = passed.send(report.cloned().map_err(str::to_owned));
        },
    );
    let driving = async {
        let mut until = async |done: &dyn Fn(&Report) -> bool| loop {
            let report = passes.recv().await.unwrap().unwrap();
            if done(&report) {
                return report;
            }
        };
        until(&|report| report.written > 0).await;
        assert_eq!(read(&checkout, "note.md"), "first\n");

        write(&checkout, "made.md", "made here\n");
        until(&|report| report.created > 0).await;
        assert_eq!(
            text_of(&peer, child(&peer, code, "made.md").await.id).await,
            "made here\n"
        );

        rewrite(&peer, note, "second\n").await;
        until(&|report| report.written > 0).await;
        assert_eq!(read(&checkout, "note.md"), "second\n");
        stop.send(()).unwrap();
    };
    let (watched, ()) = tokio::time::timeout(Duration::from_secs(120), async {
        tokio::join!(watching, driving)
    })
    .await
    .expect("the watcher saw every change");
    watched.unwrap();
    harness.stop().await;
}
