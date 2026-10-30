use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    Glyph, ICON_ARROW_LEFT, ICON_KEYBOARD, ICON_POWER, ICON_REFRESH_CW, ICON_ROTATE_CW_SQUARE,
    ICON_SLIDERS_HORIZONTAL,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::desktop;
use block_editor_beui::{Editor, HostDisplay, MediaLevels, WifiLink, WifiStatus};

use super::popup::BarPopup;
use super::power::{PowerChoice, PowerDialog, PowerRow};
use super::rotate::ScreenRotation;
use super::volume::VolumePanel;
use super::wifi::WifiPanel;
use super::{volume, wifi};
use beui::{Align, Direction, Overflow, TextAlign};
use block_editor_beui::beui::reactive::toggle_keyboard;

const SPACING: Space = Space::Lg;
const TILE_SPACING: Space = Space::Sm;
const ICON_SPACING: Space = Space::Sm;
const CAPTION_SPACING: Space = Space::Xs;
const TILE: Size = Size::Xl2;
const TILE_WIDTH: Size = Size::Xl3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Wifi,
    Rotate,
}

#[derive(Clone, Copy)]
pub(crate) struct Status {
    pub(crate) editor: Editor,
    pub(crate) wifi: Memo<WifiStatus>,
    pub(crate) wireless: Memo<bool>,
    pub(crate) levels: Memo<MediaLevels>,
    pub(crate) audible: Memo<bool>,
    pub(crate) powered: Memo<bool>,
    pub(crate) epaper: Memo<bool>,
    pub(crate) screen: Memo<Option<HostDisplay>>,
}

fn label(status: Status) -> String {
    let mut said = Vec::new();
    if status.wireless.get() {
        said.push(status.wifi.with(wifi::summary));
    }
    if status.audible.get()
        && let Some(output) = status.levels.with(|levels| levels.output)
    {
        said.push(format!("Volume: {}", volume::percent(output)));
    }
    match said.is_empty() {
        true => "Quick settings".to_owned(),
        false => format!("Quick settings ({})", said.join(", ")),
    }
}

#[component]
pub(crate) fn StatusButton(status: Status) -> NodeId {
    let Status {
        wifi,
        wireless,
        levels,
        audible,
        powered,
        ..
    } = status;
    let label = create_memo(move || label(status));
    let wifi_glyph = create_memo(move || wifi.with(wifi::glyph).to_owned());
    let volume_glyph =
        create_memo(move || volume::glyph(levels.with(|levels| levels.output)).to_owned());
    let bare = create_memo(move || !wireless.get() && !audible.get() && !powered.get());
    let page = create_signal(Page::Main);
    let shown = create_signal(false);
    let open = Binding::new(shown, move |next: bool| {
        if next {
            page.set(Page::Main);
        }
        shown.set(next);
    });
    let power = PowerChoice::new(status.editor);
    view! {
        <List spacing=Space::None>
            <BarPopup
                label
                icon_only=true
                @test_id={"desktop.status"}
                open
                leading={view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=ICON_SPACING>
                        <Show condition={wireless}>
                            <Text @test_id={"desktop.status.wifi"}>
                                <Icon glyph={wifi_glyph} />
                            </Text>
                        </Show>
                        <Show condition={audible}>
                            <Text @test_id={"desktop.status.volume"}>
                                <Icon glyph={volume_glyph} />
                            </Text>
                        </Show>
                        <Show condition={powered}>
                            <Text @test_id={"desktop.status.power"}>
                                <Icon glyph=ICON_POWER />
                            </Text>
                        </Show>
                        <Show condition={bare}>
                            <Text>
                                <Icon glyph=ICON_SLIDERS_HORIZONTAL />
                            </Text>
                        </Show>
                    </List>
                }}
            >|handle: PopoverHandle|
                <QuickSettings status={status} power={power} page={page} popover={handle} />
            </BarPopup>
            <PowerDialog choice={power} />
        </List>
    }
}

#[component]
fn QuickSettings(
    status: Status,
    power: PowerChoice,
    page: Signal<Page>,
    popover: PopoverHandle,
) -> NodeId {
    let main = create_memo(move || page.get() == Page::Main);
    let networks = create_memo(move || page.get() == Page::Wifi);
    let rotating = create_memo(move || page.get() == Page::Rotate);
    let back = Callback::new(move |()| page.set(Page::Main));
    let close = Callback::new(move |()| popover.close.call(()));
    let Status {
        editor,
        wifi,
        screen,
        ..
    } = status;
    view! {
        <List spacing=SPACING @test_id={"desktop.quick"}>
            <Show condition={main}>
                <QuickMain status={status} power={power} page={page} close={close} />
            </Show>
            <Show condition={networks}>
                <WifiPanel editor={editor} status={wifi} on_back={back} />
            </Show>
            <Show condition={rotating}>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <BackButton on_click={back} />
                        <Text style=TextStyle::HEADING>"Rotate the screen"</Text>
                    </List>
                    <ScreenRotation
                        editor={editor}
                        display={screen}
                        test_id={"desktop.quick.rotate".to_owned()}
                        on_picked={close}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn QuickMain(
    status: Status,
    power: PowerChoice,
    page: Signal<Page>,
    close: Callback<()>,
) -> NodeId {
    let Status {
        editor,
        wifi,
        wireless,
        levels,
        audible,
        powered,
        epaper,
        screen,
    } = status;
    let network = create_memo(move || {
        wifi.with(|wifi| match &wifi.link {
            WifiLink::Connected(ssid) if wifi.enabled => ssid.clone(),
            _ => "Wi-Fi".to_owned(),
        })
    });
    let wifi_on = create_memo(move || wifi.with(|wifi| wifi.enabled));
    let wifi_glyph = create_memo(move || wifi.with(wifi::glyph).to_owned());
    let rotatable = create_memo(move || screen.with(Option::is_some));
    let networks = move || {
        page.set(Page::Wifi);
        wifi::scan(editor);
    };
    let keyboard = move || {
        close.call(());
        toggle_keyboard();
    };
    let refresh = move || {
        close.call(());
        editor.act(desktop::epaper::refresh);
    };
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal wrap=true spacing=TILE_SPACING>
                <Show condition={wireless}>
                    <QuickTile
                        id="desktop.quick.wifi"
                        label={network}
                        glyph={wifi_glyph}
                        selected={wifi_on}
                        on_click={networks}
                    />
                </Show>
                <QuickTile
                    id="desktop.quick.keyboard"
                    label="Keyboard"
                    glyph=ICON_KEYBOARD
                    on_click={keyboard}
                />
                <Show condition={rotatable}>
                    <QuickTile
                        id="desktop.quick.rotate"
                        label="Rotate"
                        glyph=ICON_ROTATE_CW_SQUARE
                        on_click={move || page.set(Page::Rotate)}
                    />
                </Show>
                <Show condition={epaper}>
                    <QuickTile
                        id="desktop.quick.refresh"
                        label="Refresh"
                        glyph=ICON_REFRESH_CW
                        on_click={refresh}
                    />
                </Show>
            </List>
            <Show condition={audible}>
                <List spacing=SPACING>
                    <Separator />
                    <VolumePanel editor={editor} levels={levels} />
                </List>
            </Show>
            <Show condition={powered}>
                <List spacing=SPACING>
                    <Separator />
                    <PowerRow choice={power} on_chose={close} />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn QuickTile(
    id: &'static str,
    label: Prop<String>,
    glyph: Prop<Glyph>,
    #[prop(default = false)] selected: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let label = create_memo(move || label.get());
    let glyph = create_memo(move || glyph.get());
    view! {
        <Frame width=TILE_WIDTH>
            <List spacing=CAPTION_SPACING align=Align::Center>
                <TileButton @test_id={id} label selected outlined=true size=TILE on_click>
                    <Text size=TextSize::Lg>
                        <Icon glyph />
                    </Text>
                </TileButton>
                <Text style=TextStyle::CAPTION align=TextAlign::Center overflow=Overflow::Ellipsis>
                    {label}
                </Text>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn BackButton(on_click: Callback<()>) -> NodeId {
    view! {
        <Button
            icon_only=true
            glyph=ICON_ARROW_LEFT
            label="Back"
            variant=ButtonVariant::Ghost
            size=ButtonSize::Small
            @test_id={"desktop.quick.back"}
            on_click={move || on_click.call(())}
        />
    }
}
