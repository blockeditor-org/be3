use super::terminal::{Setup, host, serve};
use super::*;

/// A host opens its session showing `TerminalHost` (`Session::open`): a viewer there before it
/// hears it is a host as soon as it hears its presence at all (`Session::heard`), never a
/// member heard in full that shows nothing.
#[tokio::test]
async fn a_terminal_host_shows_it_is_one_from_its_first_presence() {
    tokio::task::LocalSet::new().run_until(hosted()).await;
}

async fn hosted() {
    let setup = Setup::start().await;
    let agent = setup.agent().await;
    let mut lone = host("lone-box");
    let (stop, stopped) = oneshot::channel::<()>();
    let (start, starts) = oneshot::channel::<()>();

    let (served, ()) = tokio::join!(
        async {
            let _ = starts.await;
            serve(&mut lone, agent, setup.block, stopped)
                .await
                .inspect_err(|error| eprintln!("the host failed: {error}"))
        },
        async {
            let mut viewer = setup.viewer().await;
            let own = viewer.session.client();
            start.send(()).unwrap();
            viewer
                .until("heard the host", |viewer| {
                    viewer
                        .session
                        .editors()
                        .into_iter()
                        .any(|member| Some(member) != own && viewer.session.heard(member))
                })
                .await;
            assert_eq!(
                viewer.hosts_present(),
                1,
                "what first told of the host's presence says it is a host"
            );
            viewer.leave().await;
            stop.send(()).unwrap();
        }
    );
    served.unwrap();

    setup.stop().await;
}
