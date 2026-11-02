use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use tokio::sync::mpsc::unbounded_channel;

use super::*;
use crate::dbus::test_bus::Bus;

struct FakePlayer {
    name: &'static str,
    status: String,
    heard: Mutex<Sender<String>>,
}

impl FakePlayer {
    fn hear(&self, method: &str) {
        let _ = self
            .heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(format!("{} {method}", self.name));
    }
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl FakePlayer {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.status.clone()
    }

    fn play_pause(&self) {
        self.hear("PlayPause");
    }

    fn next(&self) {
        self.hear("Next");
    }

    fn previous(&self) {
        self.hear("Previous");
    }

    fn stop(&self) {
        self.hear("Stop");
    }
}

async fn player(
    bus: &Bus,
    name: &'static str,
    status: &str,
    heard: &Sender<String>,
) -> zbus::Connection {
    let connection = bus.connect().await;
    connection
        .object_server()
        .at(
            "/org/mpris/MediaPlayer2",
            FakePlayer {
                name,
                status: status.to_owned(),
                heard: Mutex::new(heard.clone()),
            },
        )
        .await
        .expect("the player is served");
    connection
        .request_name(format!("org.mpris.MediaPlayer2.{name}"))
        .await
        .expect("the player takes its name");
    connection
}

fn until(targets: &Receiver<Option<String>>, owner: &str) {
    while targets.recv().expect("the host keeps choosing a player") != Some(owner.to_owned()) {}
}

#[test]
fn media_keys_on_a_private_bus_go_to_the_player_that_is_playing() {
    let bus = Bus::start();
    let (heard_sender, heard) = channel();
    let (target_sender, targets) = channel();
    let (requests, requested) = unbounded_channel();
    zbus::block_on(async {
        let alpha = player(&bus, "alpha", "Paused", &heard_sender).await;
        let host = bus.connect().await;
        crate::dbus::spawn(players::run_on(host, requested, move |target| {
            let _ = target_sender.send(target.map(str::to_owned));
        }));
        let alpha_owner = alpha.unique_name().expect("a name").to_string();
        until(&targets, &alpha_owner);

        let beta = player(&bus, "beta", "Playing", &heard_sender).await;
        let beta_owner = beta.unique_name().expect("a name").to_string();
        until(&targets, &beta_owner);
        for command in [
            PlayerCommand::PlayPause,
            PlayerCommand::Next,
            PlayerCommand::Previous,
            PlayerCommand::Stop,
        ] {
            let _ = requests.send(command);
        }
        for method in ["PlayPause", "Next", "Previous", "Stop"] {
            assert_eq!(
                heard.recv().expect("a player hears the key"),
                format!("beta {method}"),
                "the player that already played when it appeared is told"
            );
        }

        let served = alpha
            .object_server()
            .interface::<_, FakePlayer>("/org/mpris/MediaPlayer2")
            .await
            .expect("alpha is served");
        served.get_mut().await.status = "Playing".to_owned();
        served
            .get()
            .await
            .playback_status_changed(served.signal_emitter())
            .await
            .expect("alpha says it plays");
        until(&targets, &alpha_owner);
        let _ = requests.send(PlayerCommand::PlayPause);
        assert_eq!(
            heard.recv().expect("a player hears the key"),
            "alpha PlayPause",
            "the player that started playing last is told"
        );

        drop(served);
        alpha.close().await.expect("alpha leaves the bus");
        until(&targets, &beta_owner);
        drop(requests);
        beta.close().await.expect("beta leaves the bus");
    });
}
