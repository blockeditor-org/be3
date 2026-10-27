use beui::Overflow;
use block_editor_beui::be_block::settings::{Settings, SettingsProfile};
use block_editor_beui::be_block::{
    BlockContent, DisplaySettingsContent, InputSettingsContent, LiveEdit, SettingsContent,
    SettingsProfileContent, TagsContent, WORKSPACE_TYPE,
};
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_ARROW_LEFT, ICON_FOLDER_COG, ICON_KEYBOARD, ICON_LAYOUT_GRID, ICON_MONITOR,
    ICON_MONITOR_SMARTPHONE, ICON_PLUS, ICON_TAG,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::root_settings::is_own_settings;
use block_editor_beui::{BlockParent, BlockQuery, ContentProjection, Editor};
use settings_types::EditorSettingsContent;
use uuid::Uuid;

use super::block;
use super::devices::DevicesTable;
use super::organization::{OfflineSection, OrganizationSection};
use super::setup::{DeviceSection, DeviceSetup, DifferencesSection, SetUpDone, SetUpHeading};
use beui::{Align, Direction, ItemSize};

const PADDING: Space = Space::Xl;
const SPACING: Space = Space::Md;
const SECTION_SPACING: Space = Space::Xl3;
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
    let devices = create_signal(false);
    let here = create_memo(move || !devices.get());
    let away = create_memo(move || devices.get());
    let own_here = create_memo(move || own.get() && here.get());
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=SECTION_SPACING>
                    <Show condition={others}>
                        <Text style=TextStyle::CAPTION>{NOT_OURS}</Text>
                    </Show>
                    <Show condition={here}>
                        <Section
                            title="This device"
                            caption="A change applies on every device, unless you keep it on this one."
                        >
                            <List spacing=ROW_SPACING>
                                <Kinds
                                    editor={editor}
                                    layer={settings}
                                    blocked={blocked}
                                    shown={shown}
                                />
                                <SettingRow
                                    label="Devices and profiles"
                                    detail="What each device keeps for itself"
                                    glyph=ICON_MONITOR_SMARTPHONE
                                    @test_id={"settings.devices"}
                                    on_click={move || devices.set(true)}
                                />
                            </List>
                        </Section>
                    </Show>
                    <Show condition={here}>
                        <Workspaces editor={editor} />
                    </Show>
                    <Show condition={own_here}>
                        <Section
                            title="Tags"
                            caption="Your own tags, which only you see. A workspace's tags are in a block's menu in Files."
                        >
                            <SettingRow
                                label="Your tags"
                                detail="Put them on any block you can edit"
                                glyph=ICON_TAG
                                @test_id={"settings.personal-tags"}
                                on_click={move || {
                                    if let Some(tags) = editor.personal_tags() {
                                        editor.host().open_block(tags, TagsContent::CONTENT_TYPE);
                                    }
                                }}
                            />
                        </Section>
                    </Show>
                    <Show condition={here}>
                        <OfflineSection editor={editor} />
                    </Show>
                    <Show condition={here}>
                        <OrganizationSection editor={editor} />
                    </Show>
                    <Show condition={away}>
                        <DevicesAndProfiles
                            editor={editor}
                            settings={settings}
                            blocked={blocked}
                            shown={shown}
                            on_back={move || devices.set(false)}
                        />
                    </Show>
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn Workspaces(editor: Editor) -> NodeId {
    let roots = editor.watch_blocks(BlockQuery::Roots);
    let blocks = editor.blocks();
    let account = blocks.account_id();
    let workspaces = create_memo(move || {
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
    });
    let listed = create_memo(move || !workspaces.with(Vec::is_empty));
    view! {
        <List spacing=Space::None>
            <Show condition={listed}>
                <Section
                    title="Workspaces"
                    caption="What a workspace sets applies to everything inside it, for everyone who can view it."
                >
                    <List spacing=ROW_SPACING>
                        <ForEach keys={workspaces}>|workspace: Uuid| {
                            let label = create_memo(move || block::name_of(editor, workspace));
                            let set = create_memo(move || editor.settings_block(workspace).is_some());
                            let manages =
                                create_memo(move || editor.blocks().access(workspace).can_manage());
                            let detail = create_memo(move || match (set.get(), manages.get()) {
                                (true, _) => "Has settings of its own".to_owned(),
                                (false, true) => "Nothing set yet".to_owned(),
                                (false, false) => "Nothing set, and only its managers set them".to_owned(),
                            });
                            let unopenable = create_memo(move || !set.get() && !manages.get());
                            view! {
                                <SettingRow
                                    label={label}
                                    detail={detail}
                                    glyph=ICON_FOLDER_COG
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
fn DevicesAndProfiles(
    editor: Editor,
    settings: ContentProjection<SettingsContent>,
    blocked: Memo<bool>,
    shown: Memo<Option<Uuid>>,
    on_back: ClickCallback,
) -> NodeId {
    let block = editor.block_id();
    let profiles = settings.project(|settings| settings.root().profiles());
    let client = editor.host().client_id();
    let used = settings.project(move |settings| settings.root().profile(client));
    let children = editor.watch_blocks(BlockQuery::Children(block));
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=Space::None>
                <Button
                    label="Settings"
                    glyph=ICON_ARROW_LEFT
                    variant=ButtonVariant::Ghost
                    @test_id={"settings.back"}
                    on_click={on_back}
                />
            </List>
            <Section
                title="Where devices differ"
                caption="What each profile keeps for itself, beside what applies on all devices."
            >
                <DevicesTable editor={editor} settings={settings} />
            </Section>
            <Section
                title="Settings profiles"
                caption="Each device uses one, and starts with one of its own. What a device keeps for itself is kept in its profile."
            >
                <List spacing=ROW_SPACING>
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
                        let detail = create_memo(move || match used.get() == Some(profile) {
                            true => "Used on this device".to_owned(),
                            false => String::new(),
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
                    <Button
                        label="New settings profile"
                        glyph=ICON_PLUS
                        variant=ButtonVariant::Ghost
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
                        }}
                    />
                </List>
            </Section>
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
        <ListRow selected disabled={off} on_click={on_click}>
            <Frame padding={Padding::vertical(ROW_PADDING)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_GAP>
                    <Text color={icon_color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                    <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                        <Text color={text_color} overflow=Overflow::Ellipsis>{label}</Text>
                        <Show condition={has_detail}>
                            <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>
                                {detail}
                            </Text>
                        </Show>
                    </List>
                </List>
            </Frame>
        </ListRow>
    }
}

fn shown_block(editor: Editor) -> Memo<Option<Uuid>> {
    let focused = editor.focused_block();
    create_memo(move || focused.with(|focused| focused.block_id))
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
) -> NodeId {
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
                detail="Theme, scale, screens, sleep and lock"
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
