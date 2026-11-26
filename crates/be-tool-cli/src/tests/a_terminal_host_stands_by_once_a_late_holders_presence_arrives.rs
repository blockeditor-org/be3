use be_type_terminal::TerminalMessage;

use super::terminal::{SIZE, Setup, host, serve};
use super::*;
use crate::replica::PRESENCE_PAUSE;

/// The second host joins while what its replica says of the holder is held back: it waits on
/// the holder's presence rather than claiming the lease, and stands by once it arrives.
#[tokio::test]
async fn a_terminal_host_stands_by_once_a_late_holders_presence_arrives() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    let first_agent = setup.agent().await;
    let second_agent = setup.agent().await;
    let held = second_agent.account().as_u128();
    let mut first = host("first-box");
    let mut second = host("second-box");
    let (stop_first, first_stopped) = oneshot::channel::<()>();
    let (stop_second, second_stopped) = oneshot::channel::<()>();
    let (start_second, second_starts) = oneshot::channel::<()>();

    let (first_served, second_served, ()) = tokio::join!(
        async {
            serve(&mut first, first_agent, setup.block, first_stopped)
                .await
                .inspect_err(|error| eprintln!("the first host failed: {error}"))
        },
        async {
            let _ = second_starts.await;
            serve(&mut second, second_agent, setup.block, second_stopped)
                .await
                .inspect_err(|error| eprintln!("the second host failed: {error}"))
        },
        async {
            let mut viewer = setup.viewer().await;
            viewer
                .until("saw the first host writing", |viewer| {
                    viewer.machine().as_deref() == Some("first-box")
                })
                .await;
            viewer.send(TerminalMessage::Resize(SIZE)).await;

            be_lib_test_pause::hold(PRESENCE_PAUSE, held);
            start_second.send(()).unwrap();
            viewer
                .until("saw the second host arrive", |viewer| {
                    viewer.hosts_present() == 2
                })
                .await;
            be_lib_test_pause::reached(PRESENCE_PAUSE, held, 1).await;
            viewer.run("echo held-$((3*3))").await;
            viewer
                .until("read the first host's shell while the presence was held", |viewer| {
                    viewer.answered("held-9")
                })
                .await;
            assert_eq!(
                viewer.machine().as_deref(),
                Some("first-box"),
                "a host whose holder's presence is late does not claim the lease"
            );

            be_lib_test_pause::release(PRESENCE_PAUSE, held);
            viewer.run("echo heard-$((4*4))").await;
            viewer
                .until("read the first host's shell once the presence arrived", |viewer| {
                    viewer.answered("heard-16")
                })
                .await;
            assert_eq!(
                viewer.machine().as_deref(),
                Some("first-box"),
                "the second host stands by once the holder's presence arrives"
            );
            viewer.leave().await;
            stop_second.send(()).unwrap();
            stop_first.send(()).unwrap();
        }
    );
    first_served.unwrap();
    second_served.unwrap();

    setup.stop().await;
}
