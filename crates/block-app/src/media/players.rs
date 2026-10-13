use std::collections::HashMap;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, pin_mut};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, Message, MessageStream};

use block_plugin_api::PlayerCommand;

use super::Players;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const BUS: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";

pub(crate) struct Mpris {
    requests: UnboundedSender<PlayerCommand>,
}

impl Mpris {
    pub(crate) fn start() -> Self {
        let (requests, requested) = unbounded_channel();
        crate::dbus::spawn(run(requested));
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
    players: Vec<(String, String)>,
    recent: Option<String>,
}

impl Tracker {
    pub(super) fn appeared(&mut self, name: &str, owner: &str) {
        self.vanished(name);
        self.players.push((name.to_owned(), owner.to_owned()));
        self.players.sort();
    }

    pub(super) fn vanished(&mut self, name: &str) {
        self.players.retain(|(player, _)| player != name);
    }

    pub(super) fn status(&mut self, owner: &str, status: &str) {
        let known = self.players.iter().any(|(_, player)| player == owner);
        if known && status == "Playing" {
            self.recent = Some(owner.to_owned());
        }
    }

    pub(super) fn target(&self) -> Option<&str> {
        let recent = self
            .recent
            .as_deref()
            .filter(|recent| self.players.iter().any(|(_, owner)| owner == recent));
        recent.or_else(|| self.players.first().map(|(_, owner)| owner.as_str()))
    }
}

async fn run(mut requested: UnboundedReceiver<PlayerCommand>) {
    let Some(connection) = crate::dbus::session().await else {
        return;
    };
    let mut signals = match watch(&connection).await {
        Ok((owners, statuses)) => Some(futures_util::stream::select(owners, statuses)),
        Err(error) => {
            eprintln!("block-app: media players cannot be watched: {error}");
            None
        }
    };
    let mut tracker = Tracker::default();
    if let Err(error) = list(&connection, &mut tracker).await {
        eprintln!("block-app: media players cannot be listed: {error}");
    }
    loop {
        let next = {
            let request = requested.recv();
            let signal = async {
                match &mut signals {
                    Some(signals) => signals.next().await,
                    None => std::future::pending().await,
                }
            };
            pin_mut!(request, signal);
            match select(request, signal).await {
                Either::Left((request, _)) => Either::Left(request),
                Either::Right((signal, _)) => Either::Right(signal),
            }
        };
        match next {
            Either::Left(Some(request)) => send(&connection, &tracker, request).await,
            Either::Left(None) => return,
            Either::Right(Some(Ok(message))) => observe(&mut tracker, &message),
            Either::Right(Some(Err(error))) => {
                eprintln!("block-app: a media player signal could not be read: {error}");
            }
            Either::Right(None) => signals = None,
        }
    }
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

async fn list(connection: &Connection, tracker: &mut Tracker) -> zbus::Result<()> {
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
            tracker.status(&owner, &status);
        }
    }
    Ok(())
}

fn observe(tracker: &mut Tracker, message: &Message) {
    let header = message.header();
    match header.member().map(|member| member.as_str()) {
        Some("NameOwnerChanged") => {
            let Ok((name, _, owner)) = message.body().deserialize::<(String, String, String)>()
            else {
                return;
            };
            match owner.is_empty() {
                true => tracker.vanished(&name),
                false => tracker.appeared(&name, &owner),
            }
        }
        Some("PropertiesChanged") => {
            let Some(sender) = header.sender() else {
                return;
            };
            let Ok((_, changed, _)) =
                message
                    .body()
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            else {
                return;
            };
            let status = changed
                .get("PlaybackStatus")
                .and_then(|value| value.try_clone().ok())
                .and_then(|value| String::try_from(value).ok());
            if let Some(status) = status {
                tracker.status(sender.as_str(), &status);
            }
        }
        _ => {}
    }
}

async fn send(connection: &Connection, tracker: &Tracker, request: PlayerCommand) {
    let Some(player) = tracker.target() else {
        return;
    };
    let sent = connection
        .call_method(Some(player), PATH, Some(PLAYER), method(request), &())
        .await;
    if let Err(error) = sent {
        eprintln!("block-app: the media player did not take {request:?}: {error}");
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
