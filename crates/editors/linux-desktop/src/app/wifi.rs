use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_WIFI, ICON_WIFI_HIGH, ICON_WIFI_LOCK, ICON_WIFI_LOW, ICON_WIFI_OFF, ICON_WIFI_SYNC,
    ICON_WIFI_ZERO,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::linux_desktop;
use block_editor_beui::{Editor, WifiLink, WifiNetwork, WifiRequest, WifiSecurity, WifiStatus};

use super::popup::BarPopup;
use beui::{Align, Direction, ItemSize};

const LIST_HEIGHT: Size = Size::Custom(360.0);
const SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs2;
const MIN_PASSWORD: usize = 8;

fn bars(strength: u8, locked: bool) -> Glyph {
    match (strength, locked) {
        (0..=5, true) => ICON_WIFI_LOCK,
        (0..=5, false) => ICON_WIFI_ZERO,
        (6..=30, true) => ICON_WIFI_LOCK,
        (6..=30, false) => ICON_WIFI_LOW,
        (31..=55, true) => ICON_WIFI_LOCK,
        (31..=55, false) => ICON_WIFI_HIGH,
        (56..=80, true) => ICON_WIFI_LOCK,
        (56..=80, false) => ICON_WIFI,
        (_, true) => ICON_WIFI_LOCK,
        (_, false) => ICON_WIFI,
    }
}

fn strength_of(status: &WifiStatus, ssid: &str) -> u8 {
    status
        .networks
        .iter()
        .find(|network| network.ssid == ssid)
        .map_or(100, |network| network.strength)
}

fn glyph(status: &WifiStatus) -> Glyph {
    if !status.enabled {
        return ICON_WIFI_OFF;
    }
    match &status.link {
        WifiLink::Connected(ssid) => bars(strength_of(status, ssid), false),
        WifiLink::Connecting(_) => ICON_WIFI_SYNC,
        WifiLink::Disconnected => ICON_WIFI_OFF,
    }
}

fn summary(status: &WifiStatus) -> String {
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
            _ => "Could not connect".to_owned(),
        };
    }
    match (network.saved, network.security) {
        (true, _) => "Saved".to_owned(),
        (false, WifiSecurity::Unsupported) => "Needs a sign-in this desktop cannot do".to_owned(),
        (false, _) => String::new(),
    }
}

#[component]
pub(crate) fn WifiButton(editor: Editor, status: Memo<WifiStatus>) -> NodeId {
    let glyph = create_memo(move || status.with(glyph).to_owned());
    let label = create_memo(move || status.with(summary));
    let open = create_signal(false);
    let open = Binding::new(open, move |next: bool| {
        open.set(next);
        if next {
            editor.act(move || linux_desktop::wifi::request(WifiRequest::Scan));
        }
    });
    view! {
        <BarPopup label glyph icon_only=true @test_id={"desktop.wifi"} open>|_: PopoverHandle|
            <WifiPanel editor={editor} status={status} />
        </BarPopup>
    }
}

#[component]
fn WifiPanel(editor: Editor, status: Memo<WifiStatus>) -> NodeId {
    let enabled = create_memo(move || status.with(|status| status.enabled));
    let off = create_memo(move || !enabled.get());
    let ssids = create_memo(move || {
        status.with(|status| {
            status
                .networks
                .iter()
                .map(|network| network.ssid.clone())
                .collect::<Vec<_>>()
        })
    });
    let none = create_memo(move || enabled.get() && ssids.with(Vec::is_empty));
    let listing = create_memo(move || !ssids.with(Vec::is_empty));
    let asking = create_signal(None::<String>);
    view! {
        <List spacing=SPACING @test_id={"desktop.wifi.panel"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::HEADING>"Wi-Fi"</Text>
                <Switch
                    on={Binding::new(enabled, move |on: bool| editor.act(move || linux_desktop::wifi::request(WifiRequest::SetEnabled(on))))}
                    label="Wi-Fi"
                    @test_id={"desktop.wifi.enabled"}
                />
            </List>
            <Show condition={off}>
                <Text style=TextStyle::CAPTION>"Wi-Fi is off."</Text>
            </Show>
            <Show condition={none}>
                <Text style=TextStyle::CAPTION>"No networks found."</Text>
            </Show>
            <Show condition={listing}>
                <Scroll max_length=LIST_HEIGHT bleed={Bleed::POPOVER.top(false)}>
                    <ForEach keys={ssids}>|ssid: String|
                        <NetworkRow editor={editor} status={status} ssid asking={asking} />
                    </ForEach>
                </Scroll>
            </Show>
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
    let glyph = create_memo(move || {
        network
            .with(|network| bars(network.strength, network.security != WifiSecurity::Open))
            .to_owned()
    });
    let said = create_memo(move || {
        let network = network.get();
        status.with(|status| caption(status, &network))
    });
    let captioned = create_memo(move || !said.with(String::is_empty));
    let linked = ssid.clone();
    let connected = create_memo(move || {
        status.with(|status| status.link == WifiLink::Connected(linked.clone()))
    });
    let unreachable = create_memo(move || {
        network.with(|network| !network.saved && network.security == WifiSecurity::Unsupported)
    });
    let failed = ssid.clone();
    let wanted = ssid.clone();
    let asked = create_memo(move || {
        let retry = status.with(|status| status.failed.as_ref() == Some(&failed))
            && network.with(|network| network.security == WifiSecurity::Password);
        retry || asking.with(|asking| asking.as_ref() == Some(&wanted))
    });
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
                editor.act(move || linux_desktop::wifi::request(request));
            }
            (false, WifiSecurity::Password) => asking.set(Some(chosen.clone())),
            (false, WifiSecurity::Unsupported) => {}
        }
    };
    let id = format!("desktop.wifi.network.{ssid}");
    let disconnect_id = format!("{id}.disconnect");
    let entered = ssid.clone();
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <ListRow
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={id.clone()}
                    disabled={unreachable}
                    on_click={choose}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text size=TextSize::Lg>
                            <Icon glyph />
                        </Text>
                        <List @sizing=ItemSize::Percent(100.0) spacing=ROW_SPACING>
                            <Text>{ssid.clone()}</Text>
                            <Show condition={captioned}>
                                <Text style=TextStyle::CAPTION>{said}</Text>
                            </Show>
                        </List>
                    </List>
                </ListRow>
                <Show condition={connected}>
                    <Button
                        label="Disconnect"
                        variant=ButtonVariant::Ghost
                        @test_id={disconnect_id.clone()}
                        on_click={move || editor.act(move || linux_desktop::wifi::request(WifiRequest::Disconnect))}
                    />
                </Show>
            </List>
            <Show condition={asked}>
                <PasswordEntry editor={editor} ssid={entered.clone()} asking={asking} />
            </Show>
        </List>
    }
}

#[component]
fn PasswordEntry(editor: Editor, ssid: String, asking: Signal<Option<String>>) -> NodeId {
    let password = create_signal(String::new());
    let short = create_memo(move || password.with(|password| password.len() < MIN_PASSWORD));
    let id = format!("desktop.wifi.network.{ssid}");
    let label = format!("Password for {ssid}");
    let connect = move || {
        let typed = password.get_untracked();
        if typed.len() < MIN_PASSWORD {
            return;
        }
        password.set(String::new());
        asking.set(None);
        let request = WifiRequest::Connect {
            ssid: ssid.clone(),
            password: Some(typed),
        };
        editor.act(move || linux_desktop::wifi::request(request));
    };
    let submitted = connect.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={password}
                password=true
                focused=true
                placeholder="Password"
                label
                @test_id={format!("{id}.password")}
                on_submit={move |_: String| submitted()}
            />
            <Button
                label="Connect"
                variant=ButtonVariant::Primary
                disabled={short}
                @test_id={format!("{id}.connect")}
                on_click={connect}
            />
        </List>
    }
}
