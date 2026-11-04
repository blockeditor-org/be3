use std::collections::HashMap;
use std::ops::ControlFlow;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, pin_mut};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::message::{Flags, Type};
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, Message, MessageStream};

use be_sdk_api::PlayerCommand;

use super::Players;
use crate::dbus::Bus;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const BUS: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";

pub struct Mpris {
    requests: UnboundedSender<PlayerCommand>,
}

impl Mpris {
    pub fn start() -> Self {
        let (requests, requested) = unbounded_channel();
        crate::dbus::spawn(async move {
            serve(&crate::dbus::SESSION, requested, |_| {}).await;
        });
        Self { requests }
    }
}

impl Players for Mpris {
    fn request(&self, request: PlayerCommand) {
        let _ = self.requests.send(request);
    }
}

#[derive(Debug, Default)]
pub(super) struct Tracker {
    players: Vec<Player>,
    played: u64,
}

#[derive(Debug)]
struct Player {
    name: String,
    owner: String,
    playing: bool,
    heard: bool,
    played: u64,
}

impl Tracker {
    pub(super) fn appeared(&mut self, name: &str, owner: &str) {
        self.vanished(name);
        self.players.push(Player {
            name: name.to_owned(),
            owner: owner.to_owned(),
            playing: false,
            heard: false,
            played: 0,
        });
        self.players
            .sort_by(|player, other| player.name.cmp(&other.name));
    }

    pub(super) fn vanished(&mut self, name: &str) {
        self.players.retain(|player| player.name != name);
    }

    pub(super) fn status(&mut self, owner: &str, status: &str) {
        self.played += 1;
        let played = self.played;
        for player in self
            .players
            .iter_mut()
            .filter(|player| player.owner == owner)
        {
            let playing = status == "Playing";
            if playing && !player.playing {
                player.played = played;
            }
            player.playing = playing;
            player.heard = true;
        }
    }

    pub(super) fn first_status(&mut self, owner: &str, status: &str) {
        let unheard = self
            .players
            .iter()
            .any(|player| player.owner == owner && !player.heard);
        if unheard {
            self.status(owner, status);
        }
    }

    pub(super) fn target(&self) -> Option<&str> {
        self.players
            .iter()
            .enumerate()
            .max_by_key(|(at, player)| (player.playing, player.played, std::cmp::Reverse(*at)))
            .map(|(_, player)| player.owner.as_str())
    }
}

pub(super) async fn serve(
    bus: &Bus,
    mut requested: UnboundedReceiver<PlayerCommand>,
    mut targeted: impl FnMut(Option<&str>) + Send + 'static,
) {
    loop {
        let connection = bus.connected().await;
        let serving = run_on(connection.clone(), &mut requested, &mut targeted);
        if crate::dbus::until_closed(&connection, serving)
            .await
            .is_break()
        {
            return;
        }
    }
}

async fn run_on(
    connection: Connection,
    requested: &mut UnboundedReceiver<PlayerCommand>,
    targeted: &mut (impl FnMut(Option<&str>) + Send),
) -> ControlFlow<()> {
    let mut signals = match watch(&connection).await {
        Ok((owners, statuses)) => Some(futures_util::stream::select(owners, statuses)),
        Err(error) => {
            eprintln!("be-host: media players cannot be watched: {error}");
            None
        }
    };
    let (read_sender, mut read) = unbounded_channel();
    let mut tracker = Tracker::default();
    if let Err(error) = list(&connection, &mut tracker, &read_sender).await {
        eprintln!("be-host: media players cannot be listed: {error}");
    }
    let targeted = Untargeted(targeted);
    (targeted.0)(tracker.target());
    loop {
        let next = {
            let request = requested.recv();
            let signal = async {
                match &mut signals {
                    Some(signals) => signals.next().await,
                    None => std::future::pending().await,
                }
            };
            let status = read.recv();
            pin_mut!(request, signal, status);
            match select(request, select(signal, status)).await {
                Either::Left((request, _)) => Next::Request(request),
                Either::Right((Either::Left((signal, _)), _)) => Next::Signal(signal),
                Either::Right((Either::Right((status, _)), _)) => Next::Status(status),
            }
        };
        match next {
            Next::Request(Some(request)) => send(&connection, &tracker, request).await,
            Next::Request(None) => return ControlFlow::Break(()),
            Next::Signal(Some(Ok(message))) => {
                if let Some(owner) = observe(&mut tracker, &message) {
                    read_status(&connection, owner, &read_sender);
                }
            }
            Next::Signal(Some(Err(error))) => {
                eprintln!("be-host: a media player signal could not be read: {error}");
            }
            Next::Signal(None) => signals = None,
            Next::Status(Some((owner, status))) => tracker.first_status(&owner, &status),
            Next::Status(None) => {}
        }
        (targeted.0)(tracker.target());
    }
}

struct Untargeted<'a, T: FnMut(Option<&str>)>(&'a mut T);

impl<T: FnMut(Option<&str>)> Drop for Untargeted<'_, T> {
    fn drop(&mut self) {
        (self.0)(None);
    }
}

enum Next {
    Request(Option<PlayerCommand>),
    Signal(Option<zbus::Result<Message>>),
    Status(Option<(String, String)>),
}

async fn watch(connection: &Connection) -> zbus::Result<(MessageStream, MessageStream)> {
    let owners = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(BUS)?
        .interface(BUS)?
        .member("NameOwnerChanged")?
        .arg0ns("org.mpris.MediaPlayer2")?
        .build();
    let statuses = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(PROPERTIES)?
        .member("PropertiesChanged")?
        .path(PATH)?
        .arg(0, PLAYER)?
        .build();
    Ok((
        MessageStream::for_match_rule(owners, connection, None).await?,
        MessageStream::for_match_rule(statuses, connection, None).await?,
    ))
}

async fn list(
    connection: &Connection,
    tracker: &mut Tracker,
    read: &UnboundedSender<(String, String)>,
) -> zbus::Result<()> {
    let names: Vec<String> = connection
        .call_method(Some(BUS), BUS_PATH, Some(BUS), "ListNames", &())
        .await?
        .body()
        .deserialize()?;
    for name in names.iter().filter(|name| name.starts_with(PREFIX)) {
        let owner = connection
            .call_method(
                Some(BUS),
                BUS_PATH,
                Some(BUS),
                "GetNameOwner",
                &(name.as_str(),),
            )
            .await
            .and_then(|reply| reply.body().deserialize::<String>());
        let Ok(owner) = owner else {
            continue;
        };
        tracker.appeared(name, &owner);
        read_status(connection, owner, read);
    }
    Ok(())
}

fn read_status(connection: &Connection, owner: String, read: &UnboundedSender<(String, String)>) {
    let connection = connection.clone();
    let read = read.clone();
    crate::dbus::spawn(async move {
        let status = connection
            .call_method(
                Some(owner.as_str()),
                PATH,
                Some(PROPERTIES),
                "Get",
                &(PLAYER, "PlaybackStatus"),
            )
            .await
            .and_then(|reply| reply.body().deserialize::<OwnedValue>())
            .ok()
            .and_then(|value| String::try_from(value).ok());
        if let Some(status) = status {
            let _ = read.send((owner, status));
        }
    });
}

fn observe(tracker: &mut Tracker, message: &Message) -> Option<String> {
    let header = message.header();
    match header.member().map(|member| member.as_str()) {
        Some("NameOwnerChanged") => {
            let (name, _, owner) = message
                .body()
                .deserialize::<(String, String, String)>()
                .ok()?;
            match owner.is_empty() {
                true => {
                    tracker.vanished(&name);
                    None
                }
                false => {
                    tracker.appeared(&name, &owner);
                    Some(owner)
                }
            }
        }
        Some("PropertiesChanged") => {
            let sender = header.sender()?;
            let (_, changed, _) = message
                .body()
                .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                .ok()?;
            let status = changed
                .get("PlaybackStatus")
                .and_then(|value| value.try_clone().ok())
                .and_then(|value| String::try_from(value).ok());
            if let Some(status) = status {
                tracker.status(sender.as_str(), &status);
            }
            None
        }
        _ => None,
    }
}

async fn send(connection: &Connection, tracker: &Tracker, request: PlayerCommand) {
    let Some(player) = tracker.target() else {
        return;
    };
    let call = Message::method_call(PATH, method(request))
        .and_then(|call| call.destination(player))
        .and_then(|call| call.interface(PLAYER))
        .and_then(|call| call.with_flags(Flags::NoReplyExpected))
        .and_then(|call| call.build(&()));
    let sent = match call {
        Ok(call) => connection.send(&call).await,
        Err(error) => Err(error),
    };
    if let Err(error) = sent {
        eprintln!("be-host: the media player was not sent {request:?}: {error}");
    }
}

fn method(command: PlayerCommand) -> &'static str {
    match command {
        PlayerCommand::PlayPause => "PlayPause",
        PlayerCommand::Next => "Next",
        PlayerCommand::Previous => "Previous",
        PlayerCommand::Stop => "Stop",
    }
}
