use std::rc::Rc;

use block_editor_beui::be_block::display_settings::{LockAfter, ScreenOff};
use block_editor_beui::be_block::{DisplaySettings, DisplaySettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_RESET_SETTINGS;
use block_editor_beui::beui::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, List, Memo, ReadSignal, Show, clone,
    component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, Heading, IconButton, Scroll, Select, use_theme,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::{ContentProjection, Displays, Editor, HostDisplay, HostDisplayMode};

use super::modes::{
    LOCK_AFTER, SCREEN_OFF, Size, effective, lock_after_label, mode_label, rate_label, rates,
    saved, screen_off_label, size_label, size_of, sizes,
};

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 18.0;
const ROW_SPACING: f32 = 8.0;
const LABEL_SPACING: f32 = 4.0;
const COLUMN_SPACING: f32 = 12.0;

type Settings = Rc<ContentProjection<DisplaySettingsContent>>;

#[component]
pub fn DisplaySettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<DisplaySettingsContent>();
    let read_only = editor.read_only();
    let displays = editor.host_value::<Displays>();
    let root = settings.project(|content| content.root());
    let ids = create_memo(clone!(displays -> move || {
        displays.with(|displays| displays.iter().map(|display| display.id.clone()).collect::<Vec<String>>())
    }));
    let none = create_memo(clone!(displays -> move || displays.with(Vec::is_empty)));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <Scroll>
                <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                    <List spacing=SECTION_SPACING>
                        <List spacing=ROW_SPACING>
                            <Heading content="Displays" />
                            <Caption
                                content="Each display keeps its own mode. A display without one uses its preferred resolution at its fastest refresh rate."
                            />
                            <Show condition={none}>
                                <Caption content="No display is connected to this session." />
                            </Show>
                        </List>
                        <ScreenOffSection
                            settings={settings.clone()}
                            root={root.clone()}
                            read_only={read_only.clone()}
                        />
                        <ForEach keys={ids}>
                            {move |id: String| {
                                let (settings, root) = (settings.clone(), root.clone());
                                let (displays, read_only) = (displays.clone(), read_only.clone());
                                view! {
                                    <DisplaySection
                                        id={id}
                                        settings={settings}
                                        root={root}
                                        displays={displays}
                                        read_only={read_only}
                                    />
                                }
                            }}
                        </ForEach>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

#[component]
fn DisplaySection(
    id: String,
    settings: Settings,
    root: ReadSignal<DisplaySettings>,
    displays: Memo<Vec<HostDisplay>>,
    read_only: Memo<bool>,
) -> NodeId {
    let display = create_memo(clone!(id -> move || {
        displays.with(|displays| displays.iter().find(|display| display.id == id).cloned())
    }));
    let wanted = create_memo(clone!(id -> move || root.get().mode(&id)));
    let chosen = create_memo(clone!(display wanted -> move || {
        display.with(|display| display.as_ref().map(|display| effective(display, wanted.get())))
    }));
    let offered = create_memo(clone!(display -> move || {
        display.with(|display| display.as_ref().map(|display| display.modes.clone()).unwrap_or_default())
    }));
    let size_list = create_memo(clone!(offered -> move || offered.with(|modes| sizes(modes))));
    let rate_list = create_memo(clone!(offered chosen -> move || {
        match chosen.get() {
            Some(chosen) => offered.with(|modes| rates(modes, size_of(chosen))),
            None => Vec::new(),
        }
    }));
    let size_selected = create_memo(clone!(size_list chosen -> move || {
        let size = size_of(chosen.get()?);
        size_list.with(|sizes| sizes.iter().position(|known| *known == size))
    }));
    let rate_selected = create_memo(clone!(rate_list chosen -> move || {
        let chosen = chosen.get()?;
        rate_list.with(|rates| rates.iter().position(|known| *known == chosen))
    }));
    let name = create_memo(clone!(display -> move || {
        display.with(|display| display.as_ref().map(|display| display.name.clone()).unwrap_or_default())
    }));
    let showing = create_memo(clone!(display -> move || {
        display.with(|display| match display {
            Some(display) => format!("{}, showing {}", display.connector, mode_label(display.current)),
            None => String::new(),
        })
    }));
    let unset = create_memo(clone!(wanted -> move || wanted.get().is_none()));
    let connector = display.with_untracked(|display| {
        display
            .as_ref()
            .map(|display| display.connector.clone())
            .unwrap_or_default()
    });
    let resolution_id = format!("display-settings.{connector}.resolution");
    let refresh_id = format!("display-settings.{connector}.refresh");
    let set = Rc::new(clone!(settings id -> move |mode: Option<HostDisplayMode>| {
        settings.operate(DisplaySettings::set_mode(&id, mode.map(saved)));
    }));
    let (sized, rated, reset) = (set.clone(), set.clone(), set);
    let (size_off, rate_off) = (read_only.clone(), read_only.clone());
    let picked_size = size_list.clone();
    let picked_rate = rate_list.clone();
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
                <List @sizing=ItemSize::Percent(100.0) spacing=LABEL_SPACING>
                    <Body content={name} />
                    <Caption content={showing} />
                </List>
                <ResetButton
                    unset={unset}
                    read_only={read_only}
                    id={connector}
                    on_reset={move |_: ()| reset(None)}
                />
            </List>
            <Caption content="Resolution" />
            <Select
                options={view! {
                    <ForEach keys={size_list}>
                        {|size: Size| view! {
                            <ChoiceOption label={size_label(size)} />
                        }}
                    </ForEach>
                }}
                selected={size_selected}
                label="Resolution"
                disabled={size_off}
                @test_id={resolution_id}
                on_change={clone!(offered -> move |index: Option<usize>| {
                    let Some(size) = index.and_then(|index| picked_size.with_untracked(|sizes| sizes.get(index).copied())) else {
                        return;
                    };
                    let fastest = offered.with_untracked(|modes| rates(modes, size).first().copied());
                    if fastest.is_some() {
                        sized(fastest);
                    }
                })}
            />
            <Caption content="Refresh rate" />
            <Select
                options={view! {
                    <ForEach keys={rate_list}>
                        {|mode: HostDisplayMode| view! {
                            <ChoiceOption label={rate_label(mode.refresh_millihertz)} />
                        }}
                    </ForEach>
                }}
                selected={rate_selected}
                label="Refresh rate"
                disabled={rate_off}
                @test_id={refresh_id}
                on_change={move |index: Option<usize>| {
                    let mode = index.and_then(|index| picked_rate.with_untracked(|rates| rates.get(index).copied()));
                    if mode.is_some() {
                        rated(mode);
                    }
                }}
            />
        </List>
    }
}

#[component]
fn ScreenOffSection(
    settings: Settings,
    root: ReadSignal<DisplaySettings>,
    read_only: Memo<bool>,
) -> NodeId {
    let selected = create_memo(clone!(root -> move || {
        let chosen = root.get().screen_off();
        SCREEN_OFF.iter().position(|offered| *offered == chosen)
    }));
    let locks = create_memo(move || {
        let chosen = root.get().lock_after();
        LOCK_AFTER.iter().position(|offered| *offered == chosen)
    });
    let locking = settings.clone();
    let lock_off = read_only.clone();
    view! {
        <List spacing=ROW_SPACING>
            <Caption content="Turn off screens after" />
            <Select
                options={view! {
                    <ForEach keys={SCREEN_OFF.to_vec()}>
                        {|screen_off: ScreenOff| view! {
                            <ChoiceOption label={screen_off_label(screen_off)} />
                        }}
                    </ForEach>
                }}
                selected={selected}
                label="Turn off screens after"
                disabled={read_only}
                @test_id={"display-settings.screen-off"}
                on_change={move |index: Option<usize>| {
                    if let Some(screen_off) = index.and_then(|index| SCREEN_OFF.get(index)) {
                        settings.operate(DisplaySettings::set_screen_off(Some(*screen_off)));
                    }
                }}
            />
            <Caption content="Lock the screen after" />
            <Select
                options={view! {
                    <ForEach keys={LOCK_AFTER.to_vec()}>
                        {|lock_after: LockAfter| view! {
                            <ChoiceOption label={lock_after_label(lock_after)} />
                        }}
                    </ForEach>
                }}
                selected={locks}
                label="Lock the screen after"
                disabled={lock_off}
                @test_id={"display-settings.lock-after"}
                on_change={move |index: Option<usize>| {
                    if let Some(lock_after) = index.and_then(|index| LOCK_AFTER.get(index)) {
                        locking.operate(DisplaySettings::set_lock_after(Some(*lock_after)));
                    }
                }}
            />
        </List>
    }
}

#[component]
fn ResetButton(
    unset: Memo<bool>,
    read_only: Memo<bool>,
    id: String,
    on_reset: Callback<()>,
) -> NodeId {
    let disabled = create_memo(move || unset.get() || read_only.get());
    view! {
        <IconButton
            glyph={ICON_RESET_SETTINGS.to_owned()}
            label="Use the default"
            disabled={disabled}
            @test_id={format!("display-settings.{id}.reset")}
            on_click={move || on_reset.call(())}
        />
    }
}
