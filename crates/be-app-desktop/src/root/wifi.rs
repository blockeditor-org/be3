use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::icons::{
    ICON_LOCK, ICON_PLUS, ICON_WIFI, ICON_WIFI_HIGH, ICON_WIFI_LOW, ICON_WIFI_OFF, ICON_WIFI_SYNC,
    ICON_WIFI_ZERO,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::desktop;
use be_sdk_beui::{Editor, WifiLink, WifiNetwork, WifiRequest, WifiSecurity, WifiStatus};

use super::status::BackButton;
use beui::{Align, Direction};

const LIST_HEIGHT: Size = Size::Custom(360.0);
const SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs2;
const PASSPHRASE_LENGTHS: std::ops::RangeInclusive<usize> = 8..=64;
const WEP_KEY_LENGTHS: [usize; 4] = [5, 10, 13, 26];
const HIDDEN_SECURITIES: [WifiSecurity; 3] = [
    WifiSecurity::Password,
    WifiSecurity::Wep,
    WifiSecurity::Open,
];

fn bars(strength: u8) -> Glyph {
    match strength {
        0..=25 => ICON_WIFI_ZERO,
        26..=50 => ICON_WIFI_LOW,
        51..=75 => ICON_WIFI_HIGH,
        _ => ICON_WIFI,
    }
}

fn strength_of(status: &WifiStatus, ssid: &str) -> u8 {
    status
        .networks
        .iter()
        .find(|network| network.ssid == ssid)
        .map_or(100, |network| network.strength)
}

pub(crate) fn glyph(status: &WifiStatus) -> Glyph {
    if !status.enabled {
        return ICON_WIFI_OFF;
    }
    match &status.link {
        WifiLink::Connected(ssid) => bars(strength_of(status, ssid)),
        WifiLink::Connecting(_) => ICON_WIFI_SYNC,
        WifiLink::Disconnected => ICON_WIFI_OFF,
    }
}

pub(crate) fn summary(status: &WifiStatus) -> String {
    if !status.enabled {
        return "Wi-Fi: off".to_owned();
    }
    match &status.link {
        WifiLink::Connected(ssid) => format!("Wi-Fi: {ssid}"),
        WifiLink::Connecting(ssid) => format!("Wi-Fi: connecting to {ssid}"),
        WifiLink::Disconnected => "Wi-Fi: not connected".to_owned(),
    }
}

fn caption(status: &WifiStatus, network: &WifiNetwork) -> String {
    let ssid = &network.ssid;
    match &status.link {
        WifiLink::Connected(linked) if linked == ssid => return "Connected".to_owned(),
        WifiLink::Connecting(linked) if linked == ssid => return "Connecting…".to_owned(),
        _ => {}
    }
    if status.failed.as_ref() == Some(ssid) {
        return match network.security {
            WifiSecurity::Password => "Could not connect. Check the password.".to_owned(),
            WifiSecurity::Wep => "Could not connect. Check the key.".to_owned(),
            _ => "Could not connect".to_owned(),
        };
    }
    match (network.saved, network.security) {
        (true, _) => "Saved".to_owned(),
        (false, WifiSecurity::Wep) => "Uses WEP, an old and easily broken security".to_owned(),
        (false, WifiSecurity::Enterprise) => {
            "Needs a work or school sign-in this desktop cannot set up".to_owned()
        }
        (false, _) => String::new(),
    }
}

fn described(network: &WifiNetwork, said: &str) -> String {
    let secured = match network.security {
        WifiSecurity::Open => "open",
        _ => "secured",
    };
    let mut parts = vec![
        network.ssid.clone(),
        format!("signal {}%", network.strength),
        secured.to_owned(),
    ];
    if !said.is_empty() {
        parts.push(said.to_owned());
    }
    parts.join(", ")
}

fn ordered(status: &WifiStatus) -> Vec<String> {
    let mut ssids: Vec<String> = status
        .networks
        .iter()
        .map(|network| network.ssid.clone())
        .collect();
    let linked = match &status.link {
        WifiLink::Connected(ssid) | WifiLink::Connecting(ssid) => Some(ssid),
        WifiLink::Disconnected => None,
    };
    if let Some(index) = linked.and_then(|linked| ssids.iter().position(|ssid| ssid == linked)) {
        let first = ssids.remove(index);
        ssids.insert(0, first);
    }
    ssids
}

fn usable(security: WifiSecurity, typed: &str) -> bool {
    match security {
        WifiSecurity::Open => true,
        WifiSecurity::Password => PASSPHRASE_LENGTHS.contains(&typed.len()),
        WifiSecurity::Wep => WEP_KEY_LENGTHS.contains(&typed.len()),
        WifiSecurity::Enterprise => false,
    }
}

fn secret_name(security: WifiSecurity) -> &'static str {
    match security {
        WifiSecurity::Wep => "Key",
        _ => "Password",
    }
}

pub(crate) fn scan(editor: Editor) {
    editor.act(move || desktop::wifi::request(WifiRequest::Scan).detach());
}

#[component]
pub(crate) fn WifiPanel(editor: Editor, status: Memo<WifiStatus>, on_back: Callback<()>) -> NodeId {
    let enabled = create_memo(move || status.with(|status| status.enabled));
    let off = create_memo(move || !enabled.get());
    let ssids = create_memo(move || status.with(ordered));
    let none = create_memo(move || enabled.get() && ssids.with(Vec::is_empty));
    let asking = create_signal(None::<String>);
    let other = create_signal(false);
    view! {
        <List spacing=SPACING @test_id={"desktop.wifi.panel"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <BackButton on_click={on_back} />
                <ListItem grow=1.0>
                    <Text style=TextStyle::HEADING>"Wi-Fi"</Text>
                </ListItem>
                <Switch
                    on={Binding::new(enabled, move |on: bool| editor.act(move || desktop::wifi::request(WifiRequest::SetEnabled(on)).detach()))}
                    label="Wi-Fi"
                    @test_id={"desktop.wifi.enabled"}
                />
            </List>
            <Frame height={Length::Size(LIST_HEIGHT)}>
                <Scroll bleed={Bleed::POPOVER.top(false)}>
                    <Show condition={off}>
                        <Text style=TextStyle::CAPTION>"Wi-Fi is off."</Text>
                    </Show>
                    <Show condition={none}>
                        <Text style=TextStyle::CAPTION>"No networks found."</Text>
                    </Show>
                    <ForEach keys={ssids}>|ssid: String|
                        <NetworkRow editor={editor} status={status} ssid asking={asking} />
                    </ForEach>
                    <Show condition={enabled}>
                        <OtherNetwork editor={editor} status={status} other={other} />
                    </Show>
                </Scroll>
            </Frame>
        </List>
    }
}

#[component]
fn NetworkRow(
    editor: Editor,
    status: Memo<WifiStatus>,
    ssid: String,
    asking: Signal<Option<String>>,
) -> NodeId {
    let named = ssid.clone();
    let network = create_memo(move || {
        status.with(|status| {
            status
                .networks
                .iter()
                .find(|network| network.ssid == named)
                .cloned()
                .unwrap_or_default()
        })
    });
    let glyph = create_memo(move || bars(network.with(|network| network.strength)).to_owned());
    let secured =
        create_memo(move || network.with(|network| network.security != WifiSecurity::Open));
    let said = create_memo(move || {
        let network = network.get();
        status.with(|status| caption(status, &network))
    });
    let label =
        create_memo(move || network.with(|network| said.with(|said| described(network, said))));
    let captioned = create_memo(move || !said.with(String::is_empty));
    let linked = ssid.clone();
    let connected = create_memo(move || {
        status.with(|status| status.link == WifiLink::Connected(linked.clone()))
    });
    let unreachable = create_memo(move || {
        network.with(|network| !network.saved && network.security == WifiSecurity::Enterprise)
    });
    let failed = ssid.clone();
    let wanted = ssid.clone();
    let asked = create_memo(move || {
        let retry = status.with(|status| status.failed.as_ref() == Some(&failed))
            && network.with(|network| {
                matches!(network.security, WifiSecurity::Password | WifiSecurity::Wep)
            });
        retry || asking.with(|asking| asking.as_ref() == Some(&wanted))
    });
    let security = create_memo(move || network.with(|network| network.security));
    let chosen = ssid.clone();
    let choose = move || {
        let network = network.get_untracked();
        if connected.get_untracked() {
            return;
        }
        match (network.saved, network.security) {
            (true, _) | (false, WifiSecurity::Open) => {
                asking.set(None);
                let request = WifiRequest::Connect {
                    ssid: chosen.clone(),
                    password: None,
                };
                editor.act(move || desktop::wifi::request(request).detach());
            }
            (false, WifiSecurity::Password | WifiSecurity::Wep) => asking.set(Some(chosen.clone())),
            (false, WifiSecurity::Enterprise) => {}
        }
    };
    let id = format!("desktop.wifi.network.{ssid}");
    let disconnect_id = format!("{id}.disconnect");
    let lock_id = format!("{id}.lock");
    let entered = ssid.clone();
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <ListItem grow=1.0>
                    <ListRow @test_id={id.clone()} label disabled={unreachable} on_click={choose}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text size=TextSize::Lg>
                                <Icon glyph />
                            </Text>
                            <ListItem grow=1.0>
                                <List spacing=ROW_SPACING>
                                    <Text>{ssid.clone()}</Text>
                                    <Show condition={captioned}>
                                        <Text style=TextStyle::CAPTION>{said}</Text>
                                    </Show>
                                </List>
                            </ListItem>
                            <Show condition={secured}>
                                <Text
                                    size=TextSize::Sm
                                    color=TextColor::Muted
                                    @test_id={lock_id.clone()}
                                >
                                    <Icon glyph=ICON_LOCK />
                                </Text>
                            </Show>
                        </List>
                    </ListRow>
                </ListItem>
                <Show condition={connected}>
                    <Button
                        label="Disconnect"
                        variant=ButtonVariant::Ghost
                        @test_id={disconnect_id.clone()}
                        on_click={move || editor.act(move || desktop::wifi::request(WifiRequest::Disconnect).detach())}
                    />
                </Show>
            </List>
            <Show condition={asked}>
                <PasswordEntry
                    editor={editor}
                    ssid={entered.clone()}
                    security={security}
                    asking={asking}
                />
            </Show>
        </List>
    }
}

#[component]
fn PasswordEntry(
    editor: Editor,
    ssid: String,
    security: Memo<WifiSecurity>,
    asking: Signal<Option<String>>,
) -> NodeId {
    let password = create_signal(String::new());
    let unusable = create_memo(move || !password.with(|password| usable(security.get(), password)));
    let id = format!("desktop.wifi.network.{ssid}");
    let named = ssid.clone();
    let label = create_memo(move || format!("{} for {named}", secret_name(security.get())));
    let placeholder = create_memo(move || secret_name(security.get()).to_owned());
    let connect = move || {
        let typed = password.get_untracked();
        if !usable(security.get_untracked(), &typed) {
            return;
        }
        password.set(String::new());
        asking.set(None);
        let request = WifiRequest::Connect {
            ssid: ssid.clone(),
            password: Some(typed),
        };
        editor.act(move || desktop::wifi::request(request).detach());
    };
    let submitted = connect.clone();
    let password_test_id = format!("{id}.password");
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <ListItem grow=1.0>
                <TextInput
                    value={password}
                    kind={InputKind::Password}
                    focused=true
                    placeholder
                    label
                    @test_id={password_test_id}
                    on_submit={move |_: String| submitted()}
                />
            </ListItem>
            <Button
                label="Connect"
                variant=ButtonVariant::Primary
                disabled={unusable}
                @test_id={format!("{id}.connect")}
                on_click={connect}
            />
        </List>
    }
}

#[component]
fn OtherNetwork(editor: Editor, status: Memo<WifiStatus>, other: Signal<bool>) -> NodeId {
    let open = move || other.set(!other.get_untracked());
    view! {
        <List spacing=ROW_SPACING>
            <ListRow label="Other network…" @test_id={"desktop.wifi.other"} on_click={open}>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text size=TextSize::Lg>
                        <Icon glyph=ICON_PLUS />
                    </Text>
                    <ListItem grow=1.0>
                        <Text>"Other network…"</Text>
                    </ListItem>
                </List>
            </ListRow>
            <Show condition={other}>
                <HiddenEntry editor={editor} status={status} other={other} />
            </Show>
        </List>
    }
}

#[component]
fn HiddenEntry(editor: Editor, status: Memo<WifiStatus>, other: Signal<bool>) -> NodeId {
    let name = create_signal(String::new());
    let chosen = create_signal(0usize);
    let password = create_signal(String::new());
    let joining = create_signal(None::<String>);
    let security = create_memo(move || {
        HIDDEN_SECURITIES
            .get(chosen.get())
            .copied()
            .unwrap_or(WifiSecurity::Open)
    });
    let keyed = create_memo(move || security.get() != WifiSecurity::Open);
    let secret = create_memo(move || secret_name(security.get()).to_owned());
    let ready = create_memo(move || {
        name.with(|name| !name.is_empty())
            && password.with(|password| usable(security.get(), password))
    });
    let unready = create_memo(move || !ready.get());
    let said = create_memo(move || {
        let Some(joined) = joining.get() else {
            return String::new();
        };
        status.with(|status| match &status.link {
            WifiLink::Connecting(linked) if *linked == joined => "Connecting…".to_owned(),
            _ if status.failed.as_ref() == Some(&joined) => {
                format!("Could not connect to {joined}")
            }
            _ => String::new(),
        })
    });
    create_effect(move || {
        let joined = joining.get();
        let connected = status.with(|status| match (&status.link, &joined) {
            (WifiLink::Connected(linked), Some(joined)) => linked == joined,
            _ => false,
        });
        if connected {
            other.set(false);
        }
    });
    let join = move || {
        if !ready.get_untracked() {
            return;
        }
        let ssid = name.get_untracked();
        let security = security.get_untracked();
        let request = WifiRequest::JoinHidden {
            ssid: ssid.clone(),
            security,
            password: (security != WifiSecurity::Open).then(|| password.get_untracked()),
        };
        joining.set(Some(ssid));
        password.set(String::new());
        editor.act(move || desktop::wifi::request(request).detach());
    };
    let submitted = join.clone();
    let typed = join.clone();
    view! {
        <List spacing=SPACING @test_id={"desktop.wifi.hidden"}>
            <TextInput
                value={name}
                focused=true
                placeholder="Network name"
                label="Network name"
                @test_id={"desktop.wifi.hidden.name"}
                on_submit={move |_: String| submitted()}
            />
            <SegmentedControl
                @test_id={"desktop.wifi.hidden.security"}
                label="Security"
                options={view! {
                    <ChoiceOption label="Password" />
                    <ChoiceOption label="WEP" />
                    <ChoiceOption label="None" />
                }}
                selected={chosen}
            />
            <Show condition={keyed}>
                <TextInput
                    value={password}
                    kind={InputKind::Password}
                    placeholder={secret}
                    label={secret}
                    @test_id={"desktop.wifi.hidden.password"}
                    on_submit={move |_: String| typed()}
                />
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <ListItem grow=1.0>
                    <Text style=TextStyle::CAPTION>{said}</Text>
                </ListItem>
                <Button
                    label="Connect"
                    variant=ButtonVariant::Primary
                    disabled={unready}
                    @test_id={"desktop.wifi.hidden.connect"}
                    on_click={join}
                />
            </List>
        </List>
    }
}
