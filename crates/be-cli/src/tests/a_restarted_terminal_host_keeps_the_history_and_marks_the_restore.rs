use be_block::terminal::{TerminalMessage, TerminalStyle};

use super::terminal::{SIZE, Setup, host};
use super::*;

#[tokio::test]
async fn a_restarted_terminal_host_keeps_the_history_and_marks_the_restore() {
    let setup = Setup::start().await;
    let agent = setup.agent().await;

    let mut first = host("build-box");
    let (stop, stopped) = oneshot::channel::<()>();
    let (served, ()) = tokio::join!(
        first.serve_until(agent.clone(), setup.block, async {
            let _ = stopped.await;
        }),
        async {
            let mut viewer = setup.viewer().await;
            viewer
                .until("saw the host start writing", |viewer| {
                    viewer.host().is_some()
                })
                .await;
            viewer.send(TerminalMessage::Resize(SIZE)).await;
            viewer.run("echo before-$((1+1))").await;
            viewer
                .until("read the first shell's answer", |viewer| {
                    viewer.answered("before-2")
                })
                .await;
            viewer.leave(&setup).await;
            stop.send(()).unwrap();
        }
    );
    served.unwrap();
    drop(first);

    let mut second = host("build-box");
    let (stop, stopped) = oneshot::channel::<()>();
    let (served, ()) = tokio::join!(
        second.serve_until(agent, setup.block, async {
            let _ = stopped.await;
        }),
        async {
            let mut viewer = setup.viewer().await;
            viewer
                .until("saw the restored session", |viewer| {
                    viewer.host().is_some()
                        && viewer
                            .history()
                            .iter()
                            .any(|line| line.text.starts_with("Restored session: "))
                })
                .await;
            let history = viewer.history();
            let before = history
                .iter()
                .position(|line| line.text.ends_with("before-2") && !line.text.contains("echo"))
                .expect("the first session's output is in history");
            let marker = history
                .iter()
                .position(|line| line.text.starts_with("Restored session: "))
                .unwrap();
            assert!(before < marker, "the marker follows the old session");
            assert_eq!(
                history[marker].runs.first().map(|run| run.style),
                Some(TerminalStyle::FAINT),
                "the marker is dim"
            );
            assert_eq!(
                viewer.root().size,
                SIZE,
                "the new shell starts at the old size"
            );

            viewer.run("echo after-$((2+1))").await;
            viewer
                .until("read the new shell's answer", |viewer| {
                    viewer.answered("after-3")
                })
                .await;
            stop.send(()).unwrap();
        }
    );
    served.unwrap();

    setup.stop().await;
}
