use be_block_client::Clock;
use be_type_terminal::TerminalMessage;

use super::terminal::{SIZE, Setup, host, serve};
use super::*;
use crate::replica::PRESENCE_PAUSE;
use crate::terminal_host::Standing;

/// The second host joins while what its replica says of the holder is held back: past
/// `CLAIM_INTERVAL` it still waits on the holder's presence rather than claim the lease, and
/// stands by as soon as it arrives. Then a viewer takes the lease: the hosts hear it shows
/// nothing (`SessionMessage::Shown`) and one claims the lease back at once, the clock unmoved.
#[tokio::test]
async fn a_terminal_host_stands_by_once_a_late_holders_presence_arrives() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    let first_agent = setup.agent().await;
    let second_agent = setup.agent().await;
    let held = second_agent.account().as_u128();
    let clock = Clock::manual();
    let mut first = host("first-box").on_clock(clock.clone());
    let mut second = host("second-box").on_clock(clock.clone());
    let mut standing = second.standing();
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
            // Past the interval a claim was once timed on, the holder still unheard: what the
            // first host writes of the shell wakes the second, which steps and still waits.
            standing.mark_unchanged();
            clock.advance(crate::terminal_host::CLAIM_INTERVAL * 2);
            viewer.run("echo held-$((3*3))").await;
            viewer
                .until("read the first host's shell while the presence was held", |viewer| {
                    viewer.answered("held-9")
                })
                .await;
            standing.changed().await.unwrap();
            assert_eq!(
                *standing.borrow_and_update(),
                Standing::Waiting,
                "a host whose holder's presence is late does not claim the lease"
            );
            assert_eq!(viewer.machine().as_deref(), Some("first-box"));

            be_lib_test_pause::release(PRESENCE_PAUSE, held);
            standing
                .wait_for(|standing| *standing == Standing::StandingBy)
                .await
                .unwrap();

            // The viewer takes the lease and shows nothing: the hosts claim it back without
            // waiting out `UNHEARD_HOLDER`, which the clock never reaches.
            viewer.session.claim();
            viewer.session.settle().await;
            viewer
                .until("saw a host take the lease back", |viewer| {
                    !viewer.session.is_holder() && viewer.machine().is_some()
                })
                .await;
            viewer.leave().await;
            stop_second.send(()).unwrap();
            stop_first.send(()).unwrap();
        }
    );
    first_served.unwrap();
    second_served.unwrap();

    setup.stop().await;
}
