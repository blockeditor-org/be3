use be_block_client::Clock;
use be_block_graph::Access;

use super::terminal::{Setup, host, serve};
use super::*;

/// A viewer holds the lease, and a host whose account may only view the block is there before
/// the other by client, its claims refused. The other takes the lease from the viewer at once,
/// the clock unmoved, rather than leave it to the one that cannot take it.
#[tokio::test]
async fn a_terminal_host_without_edit_access_holds_no_other_host_off() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    let viewing_agent = setup.agent_with(Access::View).await;
    let editing_agent = setup.agent().await;
    let clock = Clock::manual();
    let mut viewing = host("viewing-box").on_clock(clock.clone());
    let mut editing = host("editing-box").on_clock(clock.clone());
    let (stop_viewing, viewing_stopped) = oneshot::channel::<()>();
    let (stop_editing, editing_stopped) = oneshot::channel::<()>();
    let (start_viewing, viewing_starts) = oneshot::channel::<()>();
    let (start_editing, editing_starts) = oneshot::channel::<()>();

    let (viewing_served, editing_served, ()) = tokio::join!(
        async {
            let _ = viewing_starts.await;
            serve(&mut viewing, viewing_agent, setup.block, viewing_stopped)
                .await
                .inspect_err(|error| eprintln!("the viewing host failed: {error}"))
        },
        async {
            let _ = editing_starts.await;
            serve(&mut editing, editing_agent, setup.block, editing_stopped)
                .await
                .inspect_err(|error| eprintln!("the editing host failed: {error}"))
        },
        async {
            let mut viewer = setup.viewer().await;
            start_viewing.send(()).unwrap();
            viewer
                .until("saw the viewing host arrive", |viewer| {
                    viewer.hosts_present() == 1
                })
                .await;
            assert!(viewer.session.is_holder(), "the viewer holds the lease");
            start_editing.send(()).unwrap();
            viewer
                .until("saw the editing host take the lease", |viewer| {
                    viewer.machine().as_deref() == Some("editing-box")
                })
                .await;
            assert!(
                viewer.client_of("viewing-box") < viewer.client_of("editing-box"),
                "the viewing host is first by client"
            );
            assert_eq!(viewer.hosts_present(), 2, "the viewing host is still there");
            viewer.leave().await;
            stop_editing.send(()).unwrap();
            stop_viewing.send(()).unwrap();
        }
    );
    viewing_served.unwrap();
    editing_served.unwrap();

    setup.stop().await;
}
