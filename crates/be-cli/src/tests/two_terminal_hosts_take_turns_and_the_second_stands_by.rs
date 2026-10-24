use be_block::presence::PresenceKind;
use be_block::terminal::{TerminalHost, TerminalMessage};

use super::terminal::{SIZE, Setup, Viewer, host};
use super::*;

fn hosts_present(viewer: &Viewer) -> usize {
    viewer
        .live
        .presence()
        .keys()
        .filter(|(_, kind)| *kind == TerminalHost::ID)
        .count()
}

#[tokio::test]
async fn two_terminal_hosts_take_turns_and_the_second_stands_by() {
    let setup = Setup::start().await;
    let first_agent = setup.agent().await;
    let second_agent = setup.agent().await;
    let mut first = host("first-box");
    let mut second = host("second-box");
    let (stop_first, first_stopped) = oneshot::channel::<()>();
    let (stop_second, second_stopped) = oneshot::channel::<()>();

    let (first_served, second_served, ()) = tokio::join!(
        async {
            first
                .serve_until(first_agent, setup.block, async {
                    let _ = first_stopped.await;
                })
                .await
                .inspect_err(|error| eprintln!("the first host failed: {error}"))
        },
        async {
            second
                .serve_until(second_agent, setup.block, async {
                    let _ = second_stopped.await;
                })
                .await
                .inspect_err(|error| eprintln!("the second host failed: {error}"))
        },
        async {
            let viewer = setup.viewer().await;
            let mut viewer = viewer;
            viewer
                .until("saw both hosts arrive", |viewer| hosts_present(viewer) == 2)
                .await;
            viewer.leave(&setup).await;
            let mut viewer = setup.viewer().await;
            viewer
                .until("saw one host writing and both present", |viewer| {
                    viewer.machine().is_some() && hosts_present(viewer) == 2
                })
                .await;
            let writer = viewer.machine().unwrap();
            viewer.send(TerminalMessage::Resize(SIZE)).await;
            viewer.run("echo first-$((3*3))").await;
            viewer
                .until("read the writing host's shell", |viewer| {
                    viewer.answered("first-9")
                })
                .await;
            assert_eq!(
                viewer.machine(),
                Some(writer.clone()),
                "the other host stands by while one writes"
            );

            let (stop, other, stop_other) = match writer.as_str() {
                "first-box" => (stop_first, "second-box", stop_second),
                _ => (stop_second, "first-box", stop_first),
            };
            stop.send(()).unwrap();
            viewer
                .until("saw the other host take over", |viewer| {
                    viewer.machine().as_deref() == Some(other)
                        && viewer
                            .history()
                            .iter()
                            .any(|line| line.text.starts_with("Restored session: "))
                })
                .await;
            assert!(viewer.answered("first-9"));
            viewer.run("echo second-$((4*4))").await;
            viewer
                .until("read the second host's shell", |viewer| {
                    viewer.answered("second-16")
                })
                .await;
            stop_other.send(()).unwrap();
        }
    );
    first_served.unwrap();
    second_served.unwrap();

    setup.stop().await;
}
