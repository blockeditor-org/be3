use terminal_types::{TerminalMessage, TerminalStyle};

use super::terminal::{SIZE, Setup, host, serve};
use super::*;

#[tokio::test]
async fn a_terminal_host_writes_its_shell_into_the_document_for_its_viewers() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    let agent = setup.agent().await;
    let mut host = host("build-box");
    let (stop, stopped) = oneshot::channel::<()>();

    let (served, ()) = tokio::join!(serve(&mut host, agent, setup.block, stopped), async {
        let mut viewer = setup.viewer().await;
        viewer
            .until("saw the host start writing", |viewer| {
                viewer.host().is_some()
            })
            .await;
        assert_eq!(viewer.machine().as_deref(), Some("build-box"));

        viewer.send(TerminalMessage::Resize(SIZE)).await;
        viewer
            .until("saw the terminal take its size", |viewer| {
                viewer.root().size == SIZE && viewer.root().screen.len() == 12
            })
            .await;
        viewer.run("echo hello-$((6*7))").await;
        viewer
            .until("read the shell's answer", |viewer| {
                viewer.answered("hello-42")
            })
            .await;

        viewer
            .run("for i in $(seq 1 30); do echo line-$i; done")
            .await;
        viewer
            .until(
                "saw the lines that scrolled away move into history",
                |viewer| {
                    let history = viewer.history();
                    history.iter().any(|line| line.text == "line-1")
                        && viewer.root().screen.iter().any(|row| row.text == "line-30")
                },
            )
            .await;
        let history = viewer.history();
        assert!(
            history.iter().all(|line| line
                .runs
                .iter()
                .all(|run| run.style != TerminalStyle::FAINT)),
            "a new session is not marked as restored"
        );
        let ones = history.iter().filter(|line| line.text == "line-1").count();
        assert_eq!(ones, 1, "each line reaches history once");

        stop.send(()).unwrap();
        viewer
            .until("saw the host leave", |viewer| viewer.host().is_none())
            .await;
    });
    served.unwrap();

    setup.stop().await;
}
