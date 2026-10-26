use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use block_plugin_api::{HostImage, HostTrayItem, TrayMenuItem, TrayRequest};
use futures_util::StreamExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::message::Type;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, MatchRule, Message, MessageStream};

use super::icons::Icons;
use super::model::{Address, Items, Properties, menu, properties, raw_entry};
use super::watcher::{self, Registered, Watcher};

const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const BUS: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";

pub(crate) enum Event {
    Registered {
        service: String,
        sender: Option<String>,
    },
    Listed(String),
    Unlisted(String),
    OwnerChanged {
        name: String,
        owner: String,
    },
    ItemSignal {
        sender: String,
        path: String,
    },
    MenuSignal {
        sender: String,
        path: String,
    },
    Fetched {
        key: String,
        ticket: u64,
        owner: Option<String>,
        properties: Box<Properties>,
        icon: Option<HostImage>,
    },
    Laid {
        key: String,
        ticket: u64,
        menu: Vec<TrayMenuItem>,
    },
    Request(TrayRequest),
}

struct Client {
    connection: Connection,
    events: UnboundedSender<Event>,
    icons: Arc<Icons>,
    items: Items,
    serving: Option<Registered>,
}

pub(crate) async fn run(
    sender: UnboundedSender<Event>,
    events: UnboundedReceiver<Event>,
    publish: impl FnMut(Vec<HostTrayItem>) + Send + 'static,
) {
    let Some(connection) = crate::dbus::session().await else {
        return;
    };
    run_on(connection, sender, events, publish).await;
}

pub(crate) async fn run_on(
    connection: Connection,
    sender: UnboundedSender<Event>,
    mut events: UnboundedReceiver<Event>,
    mut publish: impl FnMut(Vec<HostTrayItem>) + Send + 'static,
) {
    if let Err(error) = listen(&connection, &sender).await {
        eprintln!("block-app: tray items cannot be watched: {error}");
        return;
    }
    let serving = serve(&connection, &sender).await;
    let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
    if let Err(error) = connection
        .request_name_with_flags(host.as_str(), RequestNameFlags::DoNotQueue.into())
        .await
    {
        eprintln!("block-app: the tray host name cannot be taken: {error}");
    }
    match &serving {
        Some(_) => {
            if let Ok(emitter) = SignalEmitter::new(&connection, watcher::PATH) {
                let _ = Watcher::status_notifier_host_registered(&emitter).await;
            }
        }
        None => {
            if let Err(error) = join_watcher(&connection, &sender, &host).await {
                eprintln!("block-app: the running tray watcher cannot be used: {error}");
            }
        }
    }
    let mut client = Client {
        connection,
        events: sender,
        icons: Arc::new(Icons::new()),
        items: Items::default(),
        serving,
    };
    while let Some(event) = events.recv().await {
        if client.handle(event).await {
            publish(client.items.published());
        }
    }
}

async fn serve(connection: &Connection, events: &UnboundedSender<Event>) -> Option<Registered> {
    let registered = Registered::default();
    let watcher = Watcher {
        events: events.clone(),
        registered: Arc::clone(&registered),
    };
    match connection.object_server().at(watcher::PATH, watcher).await {
        Ok(true) => {}
        Ok(false) => return None,
        Err(error) => {
            eprintln!("block-app: the tray watcher cannot be served: {error}");
            return None;
        }
    }
    let reply = connection
        .request_name_with_flags(watcher::NAME, RequestNameFlags::DoNotQueue.into())
        .await;
    match reply {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Some(registered),
        reply => {
            if let Err(error) = reply {
                eprintln!("block-app: the tray watcher cannot be served: {error}");
            }
            let _ = connection
                .object_server()
                .remove::<Watcher, _>(watcher::PATH)
                .await;
            None
        }
    }
}

async fn listen(connection: &Connection, events: &UnboundedSender<Event>) -> zbus::Result<()> {
    let owners = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(BUS)?
        .interface(BUS)?
        .member("NameOwnerChanged")?
        .build();
    let items = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(ITEM)?
        .build();
    let menus = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(MENU)?
        .build();
    for (rule, read) in [
        (owners, owner_changed as fn(&Message) -> Option<Event>),
        (items, item_signal),
        (menus, menu_signal),
    ] {
        let stream = MessageStream::for_match_rule(rule, connection, None).await?;
        crate::dbus::spawn(forward(stream, events.clone(), read));
    }
    Ok(())
}

async fn forward(
    mut stream: MessageStream,
    events: UnboundedSender<Event>,
    read: fn(&Message) -> Option<Event>,
) {
    while let Some(message) = stream.next().await {
        let Ok(message) = message else {
            continue;
        };
        if let Some(event) = read(&message)
            && events.send(event).is_err()
        {
            return;
        }
    }
}

fn owner_changed(message: &Message) -> Option<Event> {
    let (name, _, owner) = message
        .body()
        .deserialize::<(String, String, String)>()
        .ok()?;
    Some(Event::OwnerChanged { name, owner })
}

fn sender_and_path(message: &Message) -> Option<(String, String)> {
    let header = message.header();
    Some((header.sender()?.to_string(), header.path()?.to_string()))
}

fn item_signal(message: &Message) -> Option<Event> {
    let (sender, path) = sender_and_path(message)?;
    Some(Event::ItemSignal { sender, path })
}

fn menu_signal(message: &Message) -> Option<Event> {
    let header = message.header();
    let member = header.member()?;
    if !matches!(member.as_str(), "LayoutUpdated" | "ItemsPropertiesUpdated") {
        return None;
    }
    let (sender, path) = sender_and_path(message)?;
    Some(Event::MenuSignal { sender, path })
}

fn watcher_signal(message: &Message) -> Option<Event> {
    let header = message.header();
    let member = header.member()?.to_string();
    let service = message.body().deserialize::<String>().ok()?;
    match member.as_str() {
        "StatusNotifierItemRegistered" => Some(Event::Listed(service)),
        "StatusNotifierItemUnregistered" => Some(Event::Unlisted(service)),
        _ => None,
    }
}

async fn join_watcher(
    connection: &Connection,
    events: &UnboundedSender<Event>,
    host: &str,
) -> zbus::Result<()> {
    let signals = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(watcher::NAME)?
        .interface(watcher::NAME)?
        .build();
    let stream = MessageStream::for_match_rule(signals, connection, None).await?;
    crate::dbus::spawn(forward(stream, events.clone(), watcher_signal));
    connection
        .call_method(
            Some(watcher::NAME),
            watcher::PATH,
            Some(watcher::NAME),
            "RegisterStatusNotifierHost",
            &(host,),
        )
        .await?;
    let listed: OwnedValue = connection
        .call_method(
            Some(watcher::NAME),
            watcher::PATH,
            Some(PROPERTIES),
            "Get",
            &(watcher::NAME, "RegisteredStatusNotifierItems"),
        )
        .await?
        .body()
        .deserialize()?;
    let listed = Vec::<String>::try_from(listed).unwrap_or_default();
    for service in listed {
        let _ = events.send(Event::Listed(service));
    }
    Ok(())
}

impl Client {
    async fn handle(&mut self, event: Event) -> bool {
        match event {
            Event::Registered { service, sender } => {
                let Some(address) = Address::registered(&service, sender.as_deref()) else {
                    return false;
                };
                let key = address.key();
                let owner = sender.unwrap_or_default();
                if self.items.add(address, owner) {
                    self.announce(&key, true).await;
                }
                self.fetch(&key);
                false
            }
            Event::Listed(service) => {
                let Some(address) = Address::listed(&service) else {
                    return false;
                };
                let key = address.key();
                let owner = match address.bus.starts_with(':') {
                    true => address.bus.clone(),
                    false => String::new(),
                };
                self.items.add(address, owner);
                self.fetch(&key);
                false
            }
            Event::Unlisted(service) => {
                Address::listed(&service).is_some_and(|address| self.items.remove(&address.key()))
            }
            Event::OwnerChanged { name, owner } => {
                if !owner.is_empty() {
                    return false;
                }
                let gone = self.items.vanished(&name);
                for key in &gone {
                    self.announce(key, false).await;
                }
                !gone.is_empty()
            }
            Event::ItemSignal { sender, path } => {
                for key in self.items.signalled(&sender, &path) {
                    self.fetch(&key);
                }
                false
            }
            Event::MenuSignal { sender, path } => {
                for key in self.items.menu_signalled(&sender, &path) {
                    self.fetch_menu(&key);
                }
                false
            }
            Event::Fetched {
                key,
                ticket,
                owner,
                properties,
                icon,
            } => {
                let Some(item) = self.items.get_mut(&key) else {
                    return false;
                };
                if item.asked != ticket {
                    return false;
                }
                if let Some(owner) = owner {
                    item.owner = owner;
                }
                let menu_moved =
                    item.menu_path() != properties.menu.as_deref() || item.properties.is_none();
                item.properties = Some(*properties);
                item.icon = icon;
                if menu_moved {
                    item.menu.clear();
                    if item.menu_path().is_some() {
                        self.fetch_menu(&key);
                    }
                }
                true
            }
            Event::Laid { key, ticket, menu } => {
                let Some(item) = self.items.get_mut(&key) else {
                    return false;
                };
                if item.menu_asked != ticket || item.menu == menu {
                    return false;
                }
                item.menu = menu;
                true
            }
            Event::Request(request) => {
                self.request(request);
                false
            }
        }
    }

    async fn announce(&self, key: &str, registered: bool) {
        let Some(list) = &self.serving else {
            return;
        };
        {
            let mut list = list.lock().unwrap_or_else(PoisonError::into_inner);
            list.retain(|listed| listed != key);
            if registered {
                list.push(key.to_owned());
            }
        }
        let Ok(emitter) = SignalEmitter::new(&self.connection, watcher::PATH) else {
            return;
        };
        let sent = match registered {
            true => Watcher::status_notifier_item_registered(&emitter, key).await,
            false => Watcher::status_notifier_item_unregistered(&emitter, key).await,
        };
        if let Err(error) = sent {
            eprintln!("block-app: a tray watcher signal was not sent: {error}");
        }
    }

    fn fetch(&mut self, key: &str) {
        let Some(item) = self.items.get_mut(key) else {
            return;
        };
        item.asked += 1;
        let ticket = item.asked;
        let (bus, owner, path) = (
            item.address.bus.clone(),
            item.owner.clone(),
            item.address.path.clone(),
        );
        let (connection, events, icons, key) = (
            self.connection.clone(),
            self.events.clone(),
            Arc::clone(&self.icons),
            key.to_owned(),
        );
        crate::dbus::spawn(async move {
            let resolved = match owner.is_empty() {
                true => name_owner(&connection, &bus).await,
                false => None,
            };
            let destination = resolved.as_deref().unwrap_or(&bus);
            let reply = connection
                .call_method(
                    Some(destination),
                    path.as_str(),
                    Some(PROPERTIES),
                    "GetAll",
                    &(ITEM,),
                )
                .await;
            let map = match reply
                .and_then(|reply| reply.body().deserialize::<HashMap<String, OwnedValue>>())
            {
                Ok(map) => map,
                Err(error) => {
                    eprintln!("block-app: the tray item {key} cannot be read: {error}");
                    return;
                }
            };
            let properties = properties(&map);
            let icon = icons.icon(&properties);
            let _ = events.send(Event::Fetched {
                key,
                ticket,
                owner: resolved,
                properties: Box::new(properties),
                icon,
            });
        });
    }

    fn fetch_menu(&mut self, key: &str) {
        let Some(item) = self.items.get_mut(key) else {
            return;
        };
        let Some(path) = item.menu_path().map(str::to_owned) else {
            return;
        };
        item.menu_asked += 1;
        let ticket = item.menu_asked;
        let destination = destination(item);
        let (connection, events, key) =
            (self.connection.clone(), self.events.clone(), key.to_owned());
        crate::dbus::spawn(async move {
            let reply = connection
                .call_method(
                    Some(destination.as_str()),
                    path.as_str(),
                    Some(MENU),
                    "GetLayout",
                    &(0_i32, -1_i32, Vec::<String>::new()),
                )
                .await;
            type Layout = (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>));
            let layout = reply.and_then(|reply| reply.body().deserialize::<Layout>());
            let (_, (id, properties, children)) = match layout {
                Ok(layout) => layout,
                Err(error) => {
                    eprintln!("block-app: the menu of the tray item {key} cannot be read: {error}");
                    return;
                }
            };
            let root = raw_entry(
                id,
                properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), &**value)),
                children.iter().map(|child| &**child),
                0,
            );
            let _ = events.send(Event::Laid {
                key,
                ticket,
                menu: menu(&root),
            });
        });
    }

    fn request(&self, request: TrayRequest) {
        let key = match &request {
            TrayRequest::Activate(key)
            | TrayRequest::SecondaryActivate(key)
            | TrayRequest::MenuOpened(key)
            | TrayRequest::MenuClosed(key) => key,
            TrayRequest::Scroll { item, .. } | TrayRequest::MenuChosen { item, .. } => item,
        };
        let Some(item) = self.items.get(key) else {
            return;
        };
        let destination = destination(item);
        let path = item.address.path.clone();
        let menu = item.menu_path().map(str::to_owned);
        let owner = item.owner.clone();
        let (connection, events) = (self.connection.clone(), self.events.clone());
        crate::dbus::spawn(async move {
            let on = Target {
                connection: &connection,
                destination: &destination,
            };
            match (request, menu) {
                (TrayRequest::Activate(_), _) => {
                    on.call(&path, ITEM, "Activate", &(0_i32, 0_i32)).await;
                }
                (TrayRequest::SecondaryActivate(_), _) => {
                    on.call(&path, ITEM, "SecondaryActivate", &(0_i32, 0_i32))
                        .await;
                }
                (
                    TrayRequest::Scroll {
                        delta, horizontal, ..
                    },
                    _,
                ) => {
                    let orientation = match horizontal {
                        true => "horizontal",
                        false => "vertical",
                    };
                    on.call(&path, ITEM, "Scroll", &(delta, orientation)).await;
                }
                (TrayRequest::MenuOpened(_), None) => {
                    on.call(&path, ITEM, "ContextMenu", &(0_i32, 0_i32)).await;
                }
                (TrayRequest::MenuOpened(_), Some(menu)) => {
                    let stale = on
                        .call(&menu, MENU, "AboutToShow", &(0_i32,))
                        .await
                        .and_then(|reply| reply.body().deserialize::<bool>().ok())
                        .unwrap_or(false);
                    if stale {
                        let _ = events.send(Event::MenuSignal {
                            sender: owner,
                            path: menu.clone(),
                        });
                    }
                    on.event(&menu, 0, "opened").await;
                }
                (TrayRequest::MenuClosed(_), Some(menu)) => {
                    on.event(&menu, 0, "closed").await;
                }
                (TrayRequest::MenuChosen { entry, .. }, Some(menu)) => {
                    on.event(&menu, entry, "clicked").await;
                }
                (TrayRequest::MenuClosed(_) | TrayRequest::MenuChosen { .. }, None) => {}
            }
        });
    }
}

struct Target<'a> {
    connection: &'a Connection,
    destination: &'a str,
}

impl Target<'_> {
    async fn call<B>(&self, path: &str, interface: &str, method: &str, body: &B) -> Option<Message>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        let sent = self
            .connection
            .call_method(Some(self.destination), path, Some(interface), method, body)
            .await;
        if let Err(error) = &sent {
            eprintln!("block-app: the tray item did not take {method}: {error}");
        }
        sent.ok()
    }

    async fn event(&self, menu: &str, id: i32, event: &str) {
        self.call(menu, MENU, "Event", &(id, event, Value::from(0_i32), 0_u32))
            .await;
    }
}

fn destination(item: &super::model::Item) -> String {
    match item.owner.is_empty() {
        true => item.address.bus.clone(),
        false => item.owner.clone(),
    }
}

async fn name_owner(connection: &Connection, name: &str) -> Option<String> {
    if name.starts_with(':') {
        return Some(name.to_owned());
    }
    connection
        .call_method(Some(BUS), BUS_PATH, Some(BUS), "GetNameOwner", &(name,))
        .await
        .ok()?
        .body()
        .deserialize::<String>()
        .ok()
}
