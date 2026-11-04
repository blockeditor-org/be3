use be_sdk_beui::be_block::settings::{Settings, SettingsProfile};
use be_sdk_beui::be_block::{
    BlockContent, DisplaySettings, DisplaySettingsContent, InputSettingsContent, LiveEdit,
    OfflineSettings, OfflineSettingsContent, SettingsContent, SettingsProfileContent, TagsContent,
    WORKSPACE_TYPE,
};
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::icons::{
    ICON_BUILDING, ICON_CLOUD_OFF, ICON_COLUMNS_3, ICON_FOLDER_COG, ICON_HARD_DRIVE, ICON_KEYBOARD,
    ICON_LAYOUT_GRID, ICON_MONITOR, ICON_MONITOR_SMARTPHONE, ICON_PLUS, ICON_TAG,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use be_sdk_beui::root_settings::is_own_settings;
use be_sdk_beui::{BlockParent, BlockQuery, ContentProjection, Editor, LayeredSettings};
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
    let others = create_memo(move || !own.get());
    let blocked = blocked(editor, settings, own);
    let blocks = editor.blocks();
    let _graph = store((
        blocks.watch(BlockQuery::Roots),
        blocks.watch(BlockQuery::Children(block)),
    ));
    let shown = shown_block(editor);
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=SECTION_SPACING>
                    <Show condition={others}>
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
                        </List>
                    </Section>
                    <Devices editor={editor} settings={settings} blocked={blocked} shown={shown} />
                    <Workspaces editor={editor} shown={shown} />
                    <Show condition={own}>
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

#[component]
fn OfflineRow(
    editor: Editor,
    settings: ContentProjection<SettingsContent>,
    blocked: Memo<bool>,
    shown: Memo<Option<Uuid>>,
) -> NodeId {
    let (open, offline) = opener::<SettingsContent, OfflineSettingsContent>(editor, settings);
    let selected = create_memo(move || {
        offline
            .get()
            .is_some_and(|block| shown.get() == Some(block))
    });
    let workspaces = workspaces(editor);
    let resolved = editor.resolved_settings::<OfflineSettings>(editor.block_id());
    let detail = create_memo(move || {
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
    });
    view! {
        <SettingRow
            label="Offline"
            detail={detail}
            glyph=ICON_CLOUD_OFF
            selected={selected}
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
    let listed = create_memo(move || !workspaces.with(Vec::is_empty));
    view! {
        <List spacing=Space::None>
            <Show condition={listed}>
                <Section title="Workspaces" caption="">
                    <List spacing=ROW_SPACING>
                        <ForEach keys={workspaces}>|workspace: Uuid| {
                            let label = create_memo(move || block::name_of(editor, workspace));
                            let settings = create_memo(move || editor.settings_block(workspace));
                            let manages =
                                create_memo(move || editor.blocks().access(workspace).can_manage());
                            let detail = create_memo(move || match (settings.get().is_some(), manages.get()) {
                                (true, _) => "Has settings of its own".to_owned(),
                                (false, true) => "Nothing set yet".to_owned(),
                                (false, false) => "Nothing set, and only its managers set them".to_owned(),
                            });
                            let unopenable =
                                create_memo(move || settings.get().is_none() && !manages.get());
                            let selected = create_memo(move || {
                                settings.get().is_some_and(|settings| shown.get() == Some(settings))
                            });
                            view! {
                                <SettingRow
                                    label={label}
                                    detail={detail}
                                    glyph=ICON_FOLDER_COG
                                    selected={selected}
                                    disabled={unopenable}
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
                <ForEach keys={profiles}>|profile: Uuid| {
                    let label = create_memo(move || {
                        children
                            .get()
                            .into_iter()
                            .flatten()
                            .find(|info| info.id == profile)
                            .and_then(|info| info.name())
                            .unwrap_or_else(|| "Profile".to_owned())
                    });
                    let class = create_memo(move || {
                        editor
                            .content_of::<SettingsProfileContent>(profile)
                            .read(|content| content.root().class)
                            .ok()
                            .flatten()
                    });
                    let detail = create_memo(move || {
                        let kind = class.get().map(|class| class.name().to_owned());
                        match (used.get() == Some(profile), kind) {
                            (true, Some(kind)) => format!("This device · {kind}"),
                            (true, None) => "This device".to_owned(),
                            (false, Some(kind)) => kind,
                            (false, None) => String::new(),
                        }
                    });
                    let selected = create_memo(move || shown.get() == Some(profile));
                    view! {
                        <SettingRow
                            label={label}
                            detail={detail}
                            glyph=ICON_MONITOR_SMARTPHONE
                            selected={selected}
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
    let listed = create_memo(move || settings.get().is_some());
    let detail = create_memo(move || {
        match settings
            .get()
            .is_some_and(|settings| editor.blocks().access(settings).can_manage())
        {
            true => "You administer them".to_owned(),
            false => "Set by its administrators".to_owned(),
        }
    });
    let selected = create_memo(move || {
        settings
            .get()
            .is_some_and(|settings| shown.get() == Some(settings))
    });
    view! {
        <List spacing=Space::None>
            <Show condition={listed}>
                <Section title="Organization" caption="">
                    <SettingRow
                        label="Policies"
                        detail={detail}
                        glyph=ICON_BUILDING
                        selected={selected}
                        @test_id={"settings.organization"}
                        on_click={move || {
                            if let Some(settings) = settings.get_untracked() {
                                editor.host().open_block_with(
                                    settings,
                                    SettingsContent::CONTENT_TYPE,
                                    ORGANIZATION_EDITOR,
                                );
                            }
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
    let others = create_memo(move || !own.get());
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
    let unused = create_memo(move || !used.get() && !blocked.get());
    let shown = shown_block(editor);
    let differs = setup.differs();
    let finished = create_signal(false);
    let setting_up =
        create_memo(move || used.get() && differs.get() && !blocked.get() && !finished.get());
    let browsing = create_memo(move || !setting_up.get());
    let status = create_memo(move || match used.get() {
        true => "Used on this device".to_owned(),
        false => "Not used on this device".to_owned(),
    });
    view! {
        <SettingsPage>
            <Show condition={others}>
                <Frame padding={Padding::horizontal(NOTICE_INSET)}>
                    <Text style=TextStyle::CAPTION>{NOT_OURS}</Text>
                </Frame>
            </Show>
            <Show condition={setting_up}>
                <SetUpHeading />
            </Show>
            <DeviceSection setup={setup} blocked={blocked} />
            <Show condition={differs}>
                <DifferencesSection setup={setup} blocked={blocked} />
            </Show>
            <Show condition={setting_up}>
                <SetUpDone on_done={move || finished.set(true)} />
            </Show>
            <Show condition={browsing}>
                <SettingsSection
                    title="Settings profile"
                    description="What is set here replaces the settings for every device, on the devices that use this profile. What is not set follows them."
                >
                    <SettingsRow label={status}>
                        <Show condition={unused}>
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
            </Show>
            <Show condition={browsing}>
                <SettingsSection title="In this profile">
                    <Frame padding=CARD_PADDING>
                        <Kinds editor={editor} layer={profile} blocked={blocked} shown={shown} />
                    </Frame>
                </SettingsSection>
            </Show>
        </SettingsPage>
    }
}

#[component]
pub(super) fn Section(title: &'static str, caption: &'static str, children: Child) -> NodeId {
    let captioned = !caption.is_empty();
    view! {
        <List spacing=SPACING>
            <List spacing=CAPTION_SPACING>
                <Text style=TextStyle::HEADING>{title}</Text>
                <Show condition={captioned}>
                    <Text style=TextStyle::CAPTION>{caption}</Text>
                </Show>
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
    on_click: ClickCallback,
) -> NodeId {
    let label = create_memo(move || label.get());
    let detail = create_memo(move || detail.get());
    let glyph = create_memo(move || glyph.get());
    let off = create_memo(move || disabled.get());
    let has_detail = create_memo(move || !detail.get().is_empty());
    let text_color = create_memo(move || match off.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let icon_color = create_memo(move || match (off.get(), selected.get()) {
        (false, true) => TextColor::Accent,
        _ => TextColor::Muted,
    });
    view! {
        <ListRow label={label} selected disabled={off} on_click={on_click}>
            <Frame padding={Padding::vertical(ROW_PADDING)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_GAP>
                    <Text color={icon_color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                    <ListItem grow=1.0>
                        <List spacing=Space::None>
                            <Text color={text_color} overflow=Overflow::Ellipsis>{label}</Text>
                            <Show condition={has_detail}>
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
    let display_detail = create_memo(move || match summarize.get() {
        true => display_now
            .with(|display| format!("{} · {}%", display.theme().name(), display.scale_percent())),
        false => "Theme, scale, screens, sleep and lock".to_owned(),
    });
    let (open_input, input) = opener::<C, InputSettingsContent>(editor, layer);
    let (open_display, display) = opener::<C, DisplaySettingsContent>(editor, layer);
    let (open_editors, editors) = opener::<C, EditorSettingsContent>(editor, layer);
    let showing = move |block: ReadSignal<Option<Uuid>>| {
        create_memo(move || block.get().is_some_and(|block| shown.get() == Some(block)))
    };
    view! {
        <List spacing=ROW_SPACING>
            <SettingRow
                label="Display"
                detail={display_detail}
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
) -> (impl Fn() + Clone + use<C, K>, ReadSignal<Option<Uuid>>) {
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
