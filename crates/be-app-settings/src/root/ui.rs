use be_block_input::InputSettingsContent;
use be_sdk_beui::be_block::settings::{Settings, SettingsProfile};
use be_sdk_beui::be_block::{
    DisplaySettings, DisplaySettingsContent, OfflineSettings, OfflineSettingsContent,
    SettingsContent, SettingsProfileContent, TagsContent, WORKSPACE_TYPE,
};
use be_sdk_beui::be_block_content::{BlockContent, LiveEdit};
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::icons::{
    ICON_BUILDING, ICON_CLOUD_OFF, ICON_COLUMNS_3, ICON_FOLDER_COG, ICON_HARD_DRIVE, ICON_KEYBOARD,
    ICON_LAYOUT_GRID, ICON_MONITOR, ICON_MONITOR_SMARTPHONE, ICON_PLUS, ICON_TAG,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use be_sdk_beui::root_settings::is_own_settings;
use be_sdk_beui::{
    BlockParent, BlockQuery, ContentProjection, Editor, LayeredSettings, PushDevice,
    PushDeviceWatch,
};
use be_type_settings::EditorSettingsContent;
use beui::Overflow;
use uuid::Uuid;

use super::block;
use super::setup::{DeviceSection, DeviceSetup, DifferencesSection, SetUpDone, SetUpHeading};
use super::{DEVICES_EDITOR, ORGANIZATION_EDITOR, STORAGE_EDITOR};
use beui::{Align, Direction};

const PADDING: Space = Space::Xl;
const SPACING: Space = Space::Md;
const SECTION_SPACING: Space = Space::Xl;
const CAPTION_SPACING: Space = Space::Xs2;
const ROW_SPACING: Space = Space::Xs2;
const ROW_PADDING: Space = Space::Xs;
const ROW_GAP: Space = Space::Lg;
const CARD_PADDING: Space = Space::Sm;
const NOTICE_INSET: Space = Space::Xl3;

const NOT_OURS: &str = "These settings belong to another account. This device never uses them, and they stay as they are.";
const ONLY_YOURS: &str = "Your organization's administrators can recover your settings, except those only you can read, such as an app's key for another service. If you lose your recovery phrase, those are lost.";

trait Layer: LiveEdit + Clone + Default + 'static {
    fn kind(&self, kind: Uuid) -> Option<Uuid>;
    fn set_kind(kind: Uuid, block: Uuid) -> Self::Op;
}

impl Layer for SettingsContent {
    fn kind(&self, kind: Uuid) -> Option<Uuid> {
        self.root().kind(kind)
    }

    fn set_kind(kind: Uuid, block: Uuid) -> Self::Op {
        Settings::set_kind(kind, block)
    }
}

impl Layer for SettingsProfileContent {
    fn kind(&self, kind: Uuid) -> Option<Uuid> {
        self.root().kind(kind)
    }

    fn set_kind(kind: Uuid, block: Uuid) -> Self::Op {
        SettingsProfile::set_kind(kind, block)
    }
}

#[component]
pub fn SettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<SettingsContent>();
    let block = editor.block_id();
    let own = create_memo(move || is_own_settings(&editor, block));
    let blocked = blocked(editor, settings, own);
    let blocks = editor.blocks();
    let _graph = store((
        blocks.watch(BlockQuery::Roots),
        blocks.watch(BlockQuery::Children(block)),
    ));
    let shown = shown_block(editor);
    let push_device = editor.watch::<PushDeviceWatch>();
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=SECTION_SPACING>
                    <Show value={move || !own.get()}>
                        <Text style=TextStyle::CAPTION>{NOT_OURS}</Text>
                    </Show>
                    <Section title="This device" caption="">
                        <List spacing=ROW_SPACING>
                            <Kinds
                                editor={editor}
                                layer={settings}
                                blocked={blocked}
                                shown={shown}
                                summarize={own}
                            />
                            <OfflineRow
                                editor={editor}
                                settings={settings}
                                blocked={blocked}
                                shown={shown}
                            />
                            <Show value={move || push_device.with(Option::is_some)}>
                                <PushRow editor={editor} device={push_device} />
                            </Show>
                        </List>
                    </Section>
                    <Devices editor={editor} settings={settings} blocked={blocked} shown={shown} />
                    <Workspaces editor={editor} shown={shown} />
                    <Show value={own}>
                        <Section title="You" caption="">
                            <List spacing=ROW_SPACING>
                                <StorageRow editor={editor} />
                                <SettingRow
                                    label="Your tags"
                                    detail="Only you see them"
                                    glyph=ICON_TAG
                                    @test_id={"settings.personal-tags"}
                                    on_click={move || {
                                        if let Some(tags) = editor.personal_tags() {
                                            editor.host().open_block(tags, TagsContent::CONTENT_TYPE);
                                        }
                                    }}
                                />
                                <Show value={move || editor.organization_settings().is_some()}>
                                    <Text style=TextStyle::CAPTION @test_id={"settings.only-yours"}>
                                        {ONLY_YOURS}
                                    </Text>
                                </Show>
                            </List>
                        </Section>
                    </Show>
                    <Organization editor={editor} shown={shown} />
                </List>
            </Frame>
        </Scroll>
    }
}

/// The way into the Storage page. It says no numbers: the page follows the server's usage
/// while it is open, and the list, open far more often, opens no stream for it.
#[component]
fn StorageRow(editor: Editor) -> NodeId {
    let block = editor.block_id();
    let selected = shown_with(editor, block, STORAGE_EDITOR);
    view! {
        <SettingRow
            label="Storage"
            detail="What your blocks take on the server"
            glyph=ICON_HARD_DRIVE
            selected={selected}
            @test_id={"settings.storage"}
            on_click={move || {
                editor
                    .host()
                    .open_block_with(block, SettingsContent::CONTENT_TYPE, STORAGE_EDITOR)
            }}
        />
    }
}

const PUSH_DENIED: &str =
    "Notifications are blocked for this site. Allow them in the browser's site settings.";

/// Whether this device shows notifications while the app is closed (plans/notifications.md),
/// for hosts that can receive pushes; the host asks for the person's permission when it is
/// turned on, so this runs in their press.
#[component]
fn PushRow(editor: Editor, device: Memo<Option<PushDevice>>) -> NodeId {
    let on = create_memo(move || device.get() == Some(PushDevice::On));
    let denied = create_memo(move || device.get() == Some(PushDevice::Denied));
    let failed = create_signal(None::<String>);
    view! {
        <SettingsRow
            label="Notifications"
            description="Hear about new messages on this device while be-host is closed"
            note={move || match (denied.get(), failed.get()) {
                (true, _) => PUSH_DENIED.to_owned(),
                (false, Some(failure)) => failure,
                (false, None) => String::new(),
            }}
            locked={denied}
            compact=true
        >
            <Switch
                on={Binding::new(on, move |on: bool| {
                    failed.set(None);
                    let asked = editor.host().set_push_device(on);
                    editor.then(asked, move |set| {
                        if let Err(failure) = set {
                            failed.set(Some(failure.to_string()));
                        }
                    });
                })}
                label="Notifications on this device"
                disabled={denied}
                @test_id={"settings.push-device"}
            />
        </SettingsRow>
    }
}

#[component]
fn OfflineRow(
    editor: Editor,
    settings: ContentProjection<SettingsContent>,
    blocked: Memo<bool>,
    shown: Memo<Option<Uuid>>,
) -> NodeId {
    let (open, offline) = opener::<SettingsContent, OfflineSettingsContent>(editor, settings);
    let workspaces = workspaces(editor);
    let resolved = editor.resolved_settings::<OfflineSettings>(editor.block_id());
    let detail = move || {
        let personal = editor.organization_settings().is_none();
        let all = workspaces.get();
        let kept = resolved.with(|resolved| {
            all.iter()
                .filter(|workspace| OfflineSettings::choice(resolved, **workspace, personal).keep)
                .count()
        });
        match all.len() {
            0 => "No workspaces yet".to_owned(),
            1 if kept == 1 => "Your workspace is kept".to_owned(),
            1 => "Your workspace is not kept".to_owned(),
            count => format!("{kept} of {count} workspaces kept"),
        }
    };
    view! {
        <SettingRow
            label="Offline"
            detail={detail}
            glyph=ICON_CLOUD_OFF
            selected={move || {
                offline
                    .get()
                    .is_some_and(|block| shown.get() == Some(block))
            }}
            disabled={blocked}
            @test_id={"settings.offline"}
            on_click={open}
        />
    }
}

pub(super) fn workspaces(editor: Editor) -> Memo<Vec<Uuid>> {
    let roots = editor.watch_blocks(BlockQuery::Roots);
    let account = editor.blocks().account_id();
    create_memo(move || {
        let mut found: Vec<Uuid> = roots
            .get()
            .into_iter()
            .flatten()
            .filter(|info| info.block_type == WORKSPACE_TYPE && info.access.can_view())
            .map(|info| info.id)
            .collect();
        found.sort_by_key(|id| {
            (
                editor.blocks().info(*id).and_then(|info| info.author) != Some(account),
                *id,
            )
        });
        found
    })
}

#[component]
fn Workspaces(editor: Editor, shown: Memo<Option<Uuid>>) -> NodeId {
    let workspaces = workspaces(editor);
    view! {
        <List spacing=Space::None>
            <Show value={move || !workspaces.with(Vec::is_empty)}>
                <Section title="Workspaces" caption="">
                    <List spacing=ROW_SPACING>
                        <ForEach value={workspaces}>|workspace: Uuid| {
                            let settings = create_memo(move || editor.settings_block(workspace));
                            let manages =
                                create_memo(move || editor.blocks().access(workspace).can_manage());
                            view! {
                                <SettingRow
                                    label={move || block::name_of(editor, workspace)}
                                    detail={move || match (settings.get().is_some(), manages.get()) {
                                        (true, _) => "Has settings of its own".to_owned(),
                                        (false, true) => "Nothing set yet".to_owned(),
                                        (false, false) => "Nothing set, and only its managers set them".to_owned(),
                                    }}
                                    glyph=ICON_FOLDER_COG
                                    selected={move || {
                                        settings.get().is_some_and(|settings| shown.get() == Some(settings))
                                    }}
                                    disabled={move || settings.get().is_none() && !manages.get()}
                                    @test_id={format!("settings.workspace.{workspace}")}
                                    on_click={move || block::open_settings_of(editor, workspace)}
                                />
                            }
                        }</ForEach>
                    </List>
                </Section>
            </Show>
        </List>
    }
}

#[component]
fn Devices(
    editor: Editor,
    settings: ContentProjection<SettingsContent>,
    blocked: Memo<bool>,
    shown: Memo<Option<Uuid>>,
) -> NodeId {
    let block = editor.block_id();
    let profiles = settings.project(|settings| settings.root().profiles());
    let client = editor.host().client_id();
    let used = settings.project(move |settings| settings.root().profile(client));
    let children = editor.watch_blocks(BlockQuery::Children(block));
    let comparing = shown_with(editor, block, DEVICES_EDITOR);
    view! {
        <Section title="Devices" caption="">
            <List spacing=ROW_SPACING>
                <SettingRow
                    label="Compare devices"
                    detail="Where each device differs"
                    glyph=ICON_COLUMNS_3
                    selected={comparing}
                    @test_id={"settings.devices"}
                    on_click={move || {
                        editor
                            .host()
                            .open_block_with(block, SettingsContent::CONTENT_TYPE, DEVICES_EDITOR)
                    }}
                />
                <ForEach value={profiles}>|profile: Uuid| {
                    let label = move || {
                        children
                            .get()
                            .into_iter()
                            .flatten()
                            .find(|info| info.id == profile)
                            .and_then(|info| info.name())
                            .unwrap_or_else(|| "Profile".to_owned())
                    };
                    let class = create_memo(move || {
                        editor
                            .content_of::<SettingsProfileContent>(profile)
                            .read(|content| content.root().class)
                            .ok()
                            .flatten()
                    });
                    let detail = move || {
                        let kind = class.get().map(|class| class.name().to_owned());
                        match (used.get() == Some(profile), kind) {
                            (true, Some(kind)) => format!("This device · {kind}"),
                            (true, None) => "This device".to_owned(),
                            (false, Some(kind)) => kind,
                            (false, None) => String::new(),
                        }
                    };
                    view! {
                        <SettingRow
                            label={label}
                            detail={detail}
                            glyph=ICON_MONITOR_SMARTPHONE
                            selected={move || shown.get() == Some(profile)}
                            @test_id={format!("settings.profile.{profile}")}
                            on_click={move || {
                                editor
                                    .host()
                                    .open_block(profile, SettingsProfileContent::CONTENT_TYPE)
                            }}
                        />
                    }
                }</ForEach>
                <SettingRow
                    label="New settings profile"
                    glyph=ICON_PLUS
                    disabled={blocked}
                    @test_id={"settings.new-profile"}
                    on_click={move || {
                        let count = settings
                            .read(|settings| settings.root().profiles().len())
                            .unwrap_or(0);
                        let profile = editor.blocks().create_named(
                            &SettingsProfileContent::default(),
                            BlockParent::Block(editor.block_id()),
                            format!("Profile {}", count + 1),
                        );
                        settings.operate(Settings::list_profile(profile));
                        editor
                            .host()
                            .open_block(profile, SettingsProfileContent::CONTENT_TYPE);
                    }}
                />
            </List>
        </Section>
    }
}

#[component]
fn Organization(editor: Editor, shown: Memo<Option<Uuid>>) -> NodeId {
    let settings = create_memo(move || editor.organization_settings());
    view! {
        <List spacing=Space::None>
            <Show value={settings}>|settings: ReadSignal<Uuid>|
                <Section title="Organization" caption="">
                    <SettingRow
                        label="Policies"
                        detail={move || {
                            match editor.blocks().access(settings.get()).can_manage() {
                                true => "You administer them".to_owned(),
                                false => "Set by its administrators".to_owned(),
                            }
                        }}
                        glyph=ICON_BUILDING
                        selected={move || shown.get() == Some(settings.get())}
                        @test_id={"settings.organization"}
                        on_click={move || {
                            editor.host().open_block_with(
                                settings.get_untracked(),
                                SettingsContent::CONTENT_TYPE,
                                ORGANIZATION_EDITOR,
                            );
                        }}
                    />
                </Section>
            </Show>
        </List>
    }
}

#[component]
pub fn SettingsProfileView(editor: Editor) -> NodeId {
    let profile = editor.block_content::<SettingsProfileContent>();
    let block = editor.block_id();
    let listed = editor.blocks().watch(BlockQuery::Block(block));
    let _roots = store(editor.blocks().watch(BlockQuery::Roots));
    let parent = create_memo(move || {
        listed
            .read()
            .ok()
            .and_then(|list| list.into_iter().next())
            .and_then(|info| info.parent?.block())
    });
    let own = create_memo(move || {
        parent
            .get()
            .is_some_and(|parent| is_own_settings(&editor, parent))
    });
    let setup = DeviceSetup::new(editor, profile, parent);
    let blocked = blocked(editor, profile, own);
    let client = editor.host().client_id();
    let used = create_memo(move || {
        parent.get().and_then(|parent| {
            editor
                .content_of::<SettingsContent>(parent)
                .read(|settings| settings.root().profile(client))
                .ok()
                .flatten()
        }) == Some(block)
    });
    let shown = shown_block(editor);
    let differs = setup.differs();
    let finished = create_signal(false);
    let setting_up =
        create_memo(move || used.get() && differs.get() && !blocked.get() && !finished.get());
    let browsing = move || {
        view! {
            <SettingsSection
                title="Settings profile"
                description="What is set here replaces the settings for every device, on the devices that use this profile. What is not set follows them."
            >
                <SettingsRow
                    label={move || match used.get() {
                        true => "Used on this device".to_owned(),
                        false => "Not used on this device".to_owned(),
                    }}
                >
                    <Show value={move || !used.get() && !blocked.get()}>
                        <Button
                            label="Use on this device"
                            variant=ButtonVariant::Primary
                            @test_id={"settings-profile.use"}
                            on_click={move || {
                                if let Some(settings) = parent.get_untracked() {
                                    editor
                                        .content_of::<SettingsContent>(settings)
                                        .operate(Settings::add_profile(client, block));
                                }
                            }}
                        />
                    </Show>
                </SettingsRow>
            </SettingsSection>
            <SettingsSection title="In this profile">
                <Frame padding=CARD_PADDING>
                    <Kinds editor={editor} layer={profile} blocked={blocked} shown={shown} />
                </Frame>
            </SettingsSection>
        }
    };
    view! {
        <SettingsPage>
            <Show value={move || !own.get()}>
                <Frame padding={Padding::horizontal(NOTICE_INSET)}>
                    <Text style=TextStyle::CAPTION>{NOT_OURS}</Text>
                </Frame>
            </Show>
            <Show value={setting_up}>
                <SetUpHeading />
            </Show>
            <DeviceSection setup={setup} blocked={blocked} />
            <Show value={differs}>
                <DifferencesSection setup={setup} blocked={blocked} />
            </Show>
            <Show value={setting_up} fallback={browsing}>
                <SetUpDone on_done={move || finished.set(true)} />
            </Show>
        </SettingsPage>
    }
}

#[component]
pub(super) fn Section(title: &'static str, caption: &'static str, children: Child) -> NodeId {
    view! {
        <List spacing=SPACING>
            <List spacing=CAPTION_SPACING>
                <Text style=TextStyle::HEADING>{title}</Text>
                {(!caption.is_empty()).then(|| view! {
                    <Text style=TextStyle::CAPTION>{caption}</Text>
                })}
            </List>
            {children}
        </List>
    }
}

#[component]
fn SettingRow(
    label: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_click: Callback<()>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let detail = create_memo(move || detail.get());
    let off = create_memo(move || disabled.get());
    let selected = create_memo(move || selected.get());
    view! {
        <ListRow label={label} selected={selected} disabled={off} on_click={on_click}>
            <Frame padding={Padding::vertical(ROW_PADDING)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_GAP>
                    <Text
                        color={move || match (off.get(), selected.get()) {
                            (false, true) => TextColor::Accent,
                            _ => TextColor::Muted,
                        }}
                        align=TextAlign::Center
                    >
                        <Icon glyph={glyph} />
                    </Text>
                    <ListItem grow=1.0>
                        <List spacing=Space::None>
                            <Text
                                color={move || match off.get() {
                                    true => TextColor::Muted,
                                    false => TextColor::Default,
                                }}
                                overflow=Overflow::Ellipsis
                            >
                                {label}
                            </Text>
                            <Show value={move || detail.with(|detail| !detail.is_empty())}>
                                <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>
                                    {detail}
                                </Text>
                            </Show>
                        </List>
                    </ListItem>
                </List>
            </Frame>
        </ListRow>
    }
}

fn shown_block(editor: Editor) -> Memo<Option<Uuid>> {
    let focused = editor.focused_block();
    create_memo(move || focused.with(|focused| focused.block_id))
}

/// Whether `block` is shown beside the list in the editor `with`, for the pages that open a
/// block with an editor of their own.
fn shown_with(editor: Editor, block: Uuid, with: Uuid) -> Memo<bool> {
    let focused = editor.focused_block();
    create_memo(move || {
        focused.with(|focused| focused.block_id == Some(block) && focused.editor == Some(with))
    })
}

fn blocked<C: Layer>(editor: Editor, layer: ContentProjection<C>, own: Memo<bool>) -> Memo<bool> {
    let loaded = layer.project(|_| true);
    let read_only = editor.read_only();
    create_memo(move || !loaded.get() || read_only.get() || !own.get())
}

#[component]
fn Kinds<C: Layer>(
    editor: Editor,
    layer: ContentProjection<C>,
    blocked: Memo<bool>,
    shown: Memo<Option<Uuid>>,
    #[prop(default = false)] summarize: Prop<bool>,
) -> NodeId {
    let display_now = LayeredSettings::<DisplaySettings>::this_device(editor).effective();
    let (open_input, input) = opener::<C, InputSettingsContent>(editor, layer);
    let (open_display, display) = opener::<C, DisplaySettingsContent>(editor, layer);
    let (open_editors, editors) = opener::<C, EditorSettingsContent>(editor, layer);
    let showing = move |block: Memo<Option<Uuid>>| {
        create_memo(move || block.get().is_some_and(|block| shown.get() == Some(block)))
    };
    view! {
        <List spacing=ROW_SPACING>
            <SettingRow
                label="Display"
                detail={move || match summarize.get() {
                    true => display_now.with(|display| {
                        format!("{} · {}%", display.theme().name(), display.scale_percent())
                    }),
                    false => "Theme, scale, screens, sleep and lock".to_owned(),
                }}
                glyph=ICON_MONITOR
                selected={showing(display)}
                disabled={blocked}
                @test_id={"settings.display"}
                on_click={open_display}
            />
            <SettingRow
                label="Input"
                detail="Keyboard, pointers and pasting"
                glyph=ICON_KEYBOARD
                selected={showing(input)}
                disabled={blocked}
                @test_id={"settings.input"}
                on_click={open_input}
            />
            <SettingRow
                label="Default editors"
                detail="Which editor opens each type"
                glyph=ICON_LAYOUT_GRID
                selected={showing(editors)}
                disabled={blocked}
                @test_id={"settings.default-editors"}
                on_click={open_editors}
            />
        </List>
    }
}

fn opener<C: Layer, K: BlockContent + Default>(
    editor: Editor,
    layer: ContentProjection<C>,
) -> (impl Fn() + Clone + use<C, K>, Memo<Option<Uuid>>) {
    let resolved = layer.project(|layer| layer.kind(K::CONTENT_TYPE));
    let host = editor.host();
    let open = move || {
        let id = resolved.get_untracked().unwrap_or_else(|| {
            let block = editor.create_child(&K::default());
            layer.operate(C::set_kind(K::CONTENT_TYPE, block));
            block
        });
        host.open_block(id, K::CONTENT_TYPE);
    };
    (open, resolved)
}
