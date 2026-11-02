use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use be_sdk_api::PlayerCommand;
use tokio::sync::mpsc::unbounded_channel;
use zbus::Connection;

use super::super::players;
use crate::dbus::test_bus::{PrivateBus, next};

struct Player {
    name: &'static str,
    heard: Mutex<Sender<String>>,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        "Playing".to_owned()
    }

    fn play_pause(&self) {
        let _ = self
            .heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(format!("{} PlayPause", self.name));
    }
}

async fn player(bus: &PrivateBus, name: &'static str, heard: &Sender<String>) -> Connection {
    let connection = bus.connect().await;
    connection
        .object_server()
        .at(
            "/org/mpris/MediaPlayer2",
            Player {
                name,
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

fn until(targets: &Receiver<Option<String>>, player: &Connection) {
    let owner = player.unique_name().expect("a name").to_string();
    while next(targets, "the host chooses the player") != Some(owner.clone()) {}
}

#[test]
fn media_keys_reach_players_again_once_the_bus_restarts() {
    let mut bus = PrivateBus::start();
    let (heard_sender, heard) = channel();
    let (target_sender, targets) = channel();
    let (requests, requested) = unbounded_channel();
    let shared = bus.shared();
    crate::dbus::spawn(async move {
        players::serve(&shared, requested, move |target| {
            let _ = target_sender.send(target.map(str::to_owned));
        })
        .await;
    });
    zbus::block_on(async {
        let alpha = player(&bus, "alpha", &heard_sender).await;
        until(&targets, &alpha);
        let _ = requests.send(PlayerCommand::PlayPause);
        assert_eq!(next(&heard, "alpha hears the key"), "alpha PlayPause");
    });

    bus.kill();
    bus.start_again();

    zbus::block_on(async {
        let beta = player(&bus, "beta", &heard_sender).await;
        until(&targets, &beta);
        let _ = requests.send(PlayerCommand::PlayPause);
        assert_eq!(next(&heard, "beta hears the key"), "beta PlayPause");
    });
}
