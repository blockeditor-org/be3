use be_block_client::Clock;
use be_type_terminal::TerminalContent;

use super::terminal::{Setup, host, serve};
use super::*;
use crate::replica::{PRESENCE_PAUSE, Session};
use crate::terminal_host::{CLAIM_INTERVAL, Standing};

/// A viewer holds the lease, and two members that may edit are there before the host by client,
/// neither ever heard from (their sessions are never driven, so they answer no one). Either may
/// be a host, so the host waits `CLAIM_INTERVAL` for each before it claims: not yet past one
/// interval and a half, and at once past two.
#[tokio::test]
async fn a_terminal_host_waits_on_each_member_before_it_not_heard_from() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    // Numbered by connecting: the two members' clients come before the host's.
    let members = [setup.agent().await, setup.agent().await];
    let agent = setup.agent().await;
    let held = agent.account().as_u128();
    let clock = Clock::manual();
    let mut third = host("third-box").on_clock(clock.clone());
    let mut standing = third.standing();
    let claims = third.claims();
    let (stop, stopped) = oneshot::channel::<()>();
    let (start, starts) = oneshot::channel::<()>();
    // What its replica says of the others waits until the viewer is heard and the clock moved.
    be_lib_test_pause::hold(PRESENCE_PAUSE, held);

    let (served, ()) = tokio::join!(
        async {
            let _ = starts.await;
            serve(&mut third, agent, setup.block, stopped)
                .await
                .inspect_err(|error| eprintln!("the host failed: {error}"))
        },
        async {
            // The viewer joins first, so it holds the lease.
            let mut viewer = setup.viewer().await;
            let mut silent = Vec::new();
            for peer in members {
                let opened = Session::<_, TerminalContent>::open(
                    peer,
                    setup.block,
                    sources()(),
                    Vec::new(),
                );
                silent.push(opened.await.unwrap());
            }
            start.send(()).unwrap();
            viewer
                .until("saw the host arrive", |viewer| viewer.hosts_present() == 1)
                .await;
            assert!(viewer.session.is_holder(), "the viewer holds the lease");
            be_lib_test_pause::reached(PRESENCE_PAUSE, held, 1).await;

            // Past one interval and a half, the viewer heard and the two members not: it waits.
            clock.advance(CLAIM_INTERVAL + CLAIM_INTERVAL / 2);
            standing.mark_unchanged();
            be_lib_test_pause::release(PRESENCE_PAUSE, held);
            standing.changed().await.unwrap();
            assert_eq!(*standing.borrow_and_update(), Standing::Waiting);
            assert_eq!(
                *claims.borrow(),
                0,
                "the host waits an interval for each member before it not heard from"
            );

            // Past two intervals it claims.
            clock.advance(CLAIM_INTERVAL);
            viewer
                .until("saw the host take the lease", |viewer| {
                    viewer.machine().as_deref() == Some("third-box")
                })
                .await;
            assert_eq!(*claims.borrow(), 1);
            viewer.leave().await;
            stop.send(()).unwrap();
            for session in silent {
                session.close().await;
            }
        }
    );
    served.unwrap();

    setup.stop().await;
}
