use be_block_client::Clock;
use be_type_terminal::TerminalMessage;

use super::terminal::{SIZE, Setup, host, serve};
use super::*;
use crate::replica::SESSION_PAUSE;
use crate::terminal_host::{CLAIM_INTERVAL, Standing, UNHEARD_HOLDER};

/// A viewer takes the lease while the host first by client is connected but stuck (held still
/// by `SESSION_PAUSE`), so it never claims the lease back. The second host leaves it to that one
/// past `CLAIM_INTERVAL`, and claims it itself once `UNHEARD_HOLDER` has passed.
#[tokio::test]
async fn a_terminal_host_leaves_the_lease_to_a_stuck_host_before_it_only_for_a_while() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    let first_agent = setup.agent().await;
    let second_agent = setup.agent().await;
    let first_key = first_agent.account().as_u128();
    let clock = Clock::manual();
    let mut first = host("first-box").on_clock(clock.clone());
    let mut second = host("second-box").on_clock(clock.clone());
    let mut standing = second.standing();
    let claims = second.claims();
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
            start_second.send(()).unwrap();
            standing
                .wait_for(|standing| *standing == Standing::StandingBy)
                .await
                .unwrap();
            viewer
                .until("saw the second host arrive", |viewer| {
                    viewer.hosts_present() == 2
                })
                .await;
            assert!(
                viewer.client_of("first-box") < viewer.client_of("second-box"),
                "the first host is first by client"
            );

            // Woken by a message, the first host comes to the pause once it has taken it, and
            // stays there while the viewer takes the lease.
            be_lib_test_pause::hold(SESSION_PAUSE, first_key);
            viewer.send(TerminalMessage::Resize(SIZE)).await;
            be_lib_test_pause::reached(SESSION_PAUSE, first_key, 1).await;
            let before = *claims.borrow();
            viewer.session.claim();
            viewer.session.settle().await;
            viewer
                .until("took the lease", |viewer| viewer.session.is_holder())
                .await;
            let seen = standing
                .wait_for(|standing| *standing != Standing::StandingBy)
                .await
                .map(|standing| *standing)
                .unwrap();
            assert_eq!(seen, Standing::Waiting);

            // Past `CLAIM_INTERVAL`, the second still leaves the lease to the first: the viewer
            // showing something wakes it, and it steps without claiming.
            standing.mark_unchanged();
            clock.advance(CLAIM_INTERVAL * 2);
            viewer.session.show(Uuid::from_u128(0xc0), Some(vec![1]));
            viewer.session.settle().await;
            standing.changed().await.unwrap();
            assert_eq!(*standing.borrow_and_update(), Standing::Waiting);
            assert_eq!(
                *claims.borrow(),
                before,
                "the second host leaves the lease to the first"
            );

            // Once `UNHEARD_HOLDER` has passed it claims the lease itself.
            clock.advance(UNHEARD_HOLDER);
            viewer
                .until("saw the second host take the lease", |viewer| {
                    viewer.machine().as_deref() == Some("second-box")
                })
                .await;
            assert_eq!(*claims.borrow(), before + 1);

            be_lib_test_pause::release(SESSION_PAUSE, first_key);
            viewer.leave().await;
            stop_second.send(()).unwrap();
            stop_first.send(()).unwrap();
        }
    );
    first_served.unwrap();
    second_served.unwrap();

    setup.stop().await;
}
