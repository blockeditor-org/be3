use terminal_types::{Key, Modifiers, TerminalMessage};

use super::terminal::{SIZE, Setup, host, serve};
use super::*;

#[tokio::test]
async fn a_terminal_host_encodes_keys_for_the_mode_its_program_set() {
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
        viewer.send(TerminalMessage::Resize(SIZE)).await;
        viewer
            .run("printf '\\033[?1h'; echo ready-$((1+1)); od -c")
            .await;
        viewer
            .until("saw the program set application cursor keys", |viewer| {
                viewer.answered("ready-2")
            })
            .await;
        viewer.key(Key::ArrowUp, Modifiers::NONE).await;
        viewer.key(Key::Enter, Modifiers::NONE).await;
        viewer
            .key(
                Key::D,
                Modifiers {
                    ctrl: true,
                    ..Modifiers::NONE
                },
            )
            .await;
        viewer
            .until(
                "saw the arrow encoded for application cursor keys",
                |viewer| viewer.text().contains("033   O   A"),
            )
            .await;
        stop.send(()).unwrap();
    });
    served.unwrap();

    setup.stop().await;
}
