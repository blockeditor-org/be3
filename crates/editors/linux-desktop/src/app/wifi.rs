use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_NETWORK_WIFI_1_BAR, ICON_NETWORK_WIFI_1_BAR_LOCKED, ICON_NETWORK_WIFI_2_BAR,
    ICON_NETWORK_WIFI_2_BAR_LOCKED, ICON_NETWORK_WIFI_3_BAR, ICON_NETWORK_WIFI_3_BAR_LOCKED,
    ICON_SIGNAL_WIFI_0_BAR, ICON_SIGNAL_WIFI_4_BAR, ICON_SIGNAL_WIFI_4_BAR_LOCK,
    ICON_SIGNAL_WIFI_OFF, ICON_SIGNAL_WIFI_STATUSBAR_NOT_CONNECTED, ICON_WIFI_FIND, ICON_WIFI_LOCK,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::PopoverHandle;
use block_editor_beui::{Editor, WifiLink, WifiNetwork, WifiRequest, WifiSecurity, WifiStatus};

use super::popup::BarPopup;

const WIFI_WIDTH: f32 = 320.0;
const LIST_HEIGHT: f32 = 360.0;
const SPACING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;
const MIN_PASSWORD: usize = 8;

fn bars(strength: u8, locked: bool) -> &'static str {
    match (strength, locked) {
        (0..=5, true) => ICON_WIFI_LOCK,
        (0..=5, false) => ICON_SIGNAL_WIFI_0_BAR,
        (6..=30, true) => ICON_NETWORK_WIFI_1_BAR_LOCKED,
        (6..=30, false) => ICON_NETWORK_WIFI_1_BAR,
        (31..=55, true) => ICON_NETWORK_WIFI_2_BAR_LOCKED,
        (31..=55, false) => ICON_NETWORK_WIFI_2_BAR,
        (56..=80, true) => ICON_NETWORK_WIFI_3_BAR_LOCKED,
        (56..=80, false) => ICON_NETWORK_WIFI_3_BAR,
        (_, true) => ICON_SIGNAL_WIFI_4_BAR_LOCK,
        (_, false) => ICON_SIGNAL_WIFI_4_BAR,
    }
}

fn strength_of(status: &WifiStatus, ssid: &str) -> u8 {
    status
        .networks
        .iter()
        .find(|network| network.ssid == ssid)
        .map_or(100, |network| network.strength)
}

fn glyph(status: &WifiStatus) -> &'static str {
    if !status.enabled {
        return ICON_SIGNAL_WIFI_OFF;
    }
    match &status.link {
        WifiLink::Connected(ssid) => bars(strength_of(status, ssid), false),
        WifiLink::Connecting(_) => ICON_WIFI_FIND,
        WifiLink::Disconnected => ICON_SIGNAL_WIFI_STATUSBAR_NOT_CONNECTED,
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
    let opened = move |open: bool| {
        if open {
            editor.act(WifiRequest::Scan);
        }
    };
    view! {
        <BarPopup
            label
            glyph
            icon_only=true
            width=WIFI_WIDTH
            @test_id={"desktop.wifi"}
            on_open_change={opened}
        >|_: PopoverHandle|
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
    let (asking, set_asking) = create_signal(None::<String>);
    view! {
        <List spacing=SPACING @test_id={"desktop.wifi.panel"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::HEADING string="Wi-Fi" />
                <Switch
                    on={enabled}
                    label="Wi-Fi"
                    @test_id={"desktop.wifi.enabled"}
                    on_change={move |on: bool| editor.act(WifiRequest::SetEnabled(on))}
                />
            </List>
            <Show condition={off}>
                <Text style=TextStyle::CAPTION string="Wi-Fi is off." />
            </Show>
            <Show condition={none}>
                <Text style=TextStyle::CAPTION string="No networks found." />
            </Show>
            <Show condition={listing}>
                <Scroll max_length=LIST_HEIGHT>
                    <ForEach keys={ssids}>|ssid: String|
                        <NetworkRow
                            editor={editor}
                            status={status}
                            ssid
                            asking={asking}
                            set_asking={set_asking}
                        />
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
    asking: ReadSignal<Option<String>>,
    set_asking: WriteSignal<Option<String>>,
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
                set_asking.set(None);
                editor.act(WifiRequest::Connect {
                    ssid: chosen.clone(),
                    password: None,
                });
            }
            (false, WifiSecurity::Password) => set_asking.set(Some(chosen.clone())),
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
                            <Text string={ssid.clone()} wrap=true />
                            <Show condition={captioned}>
                                <Text style=TextStyle::CAPTION string={said} wrap=true />
                            </Show>
                        </List>
                    </List>
                </ListRow>
                <Show condition={connected}>
                    <Button
                        label="Disconnect"
                        variant=ButtonVariant::Ghost
                        @test_id={disconnect_id.clone()}
                        on_click={move || editor.act(WifiRequest::Disconnect)}
                    />
                </Show>
            </List>
            <Show condition={asked}>
                <PasswordEntry editor={editor} ssid={entered.clone()} set_asking={set_asking} />
            </Show>
        </List>
    }
}

#[component]
fn PasswordEntry(editor: Editor, ssid: String, set_asking: WriteSignal<Option<String>>) -> NodeId {
    let (password, set_password) = create_signal(String::new());
    let short = create_memo(move || password.with(|password| password.len() < MIN_PASSWORD));
    let id = format!("desktop.wifi.network.{ssid}");
    let label = format!("Password for {ssid}");
    let connect = move || {
        let typed = password.get_untracked();
        if typed.len() < MIN_PASSWORD {
            return;
        }
        set_password.set(String::new());
        set_asking.set(None);
        editor.act(WifiRequest::Connect {
            ssid: ssid.clone(),
            password: Some(typed),
        });
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
                on_change={move |typed: String| set_password.set(typed)}
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
