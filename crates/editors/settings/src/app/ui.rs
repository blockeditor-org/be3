use block_editor_beui::be_block::settings::{Settings, SettingsProfile};
use block_editor_beui::be_block::{
    BlockContent, DisplaySettingsContent, EditorSettingsContent, InputSettingsContent, LiveEdit,
    SettingsContent, SettingsProfileContent, UiSettingsContent,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ADD, ICON_CHECK};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, List, Memo, Show, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Button, ButtonVariant, Caption, Heading, Scroll, use_theme};
use block_editor_beui::root_settings::is_own_settings;
use block_editor_beui::{BlockParent, BlockQuery, ContentProjection, Editor, Loaded};
use uuid::Uuid;

const PADDING: f32 = 20.0;
const SPACING: f32 = 10.0;

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
    let profiles = settings.project(|settings| settings.root().profiles());
    let client = editor.host().client_id();
    let used = settings.project(move |settings| settings.root().profile(client));
    let adding = settings;
    let loaded =
        editor.loaded::<SettingsContent>(&[BlockQuery::Roots, BlockQuery::Children(block)]);
    let children = editor.watch_blocks(BlockQuery::Children(block));
    let theme = use_theme();
    view! {
        <Loaded loaded={loaded}>
            <Frame color={theme.background}>
                <Scroll>
                    <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                        <List spacing=SPACING>
                            <Heading content="Settings" />
                            <Show condition={others}>
                                <Caption content=NOT_OURS />
                            </Show>
                            <Caption
                                content="Settings for every device. A settings profile can change any of them for the devices that use it."
                            />
                            <Kinds editor={editor} layer={settings} blocked={blocked} />
                            <Heading content="Settings profiles" />
                            <Caption
                                content="Each device uses one, and starts with one of its own."
                            />
                            <ForEach keys={profiles}>
                                {move |profile: Uuid| {
                                    let label = create_memo(move || {
                                        children
                                            .get()
                                            .into_iter()
                                            .flatten()
                                            .find(|info| info.id == profile)
                                            .and_then(|info| info.name())
                                            .unwrap_or_else(|| "Profile".to_owned())
                                    });
                                    let glyph = create_memo(move || match used.get() == Some(profile) {
                                        true => ICON_CHECK.to_owned(),
                                        false => String::new(),
                                    });
                                    view! {
                                        <Button
                                            label={label}
                                            glyph={glyph}
                                            variant=ButtonVariant::Secondary
                                            @test_id={format!("settings.profile.{profile}")}
                                            on_click={move || {
                                                editor
                                                    .host()
                                                    .open_block(profile, SettingsProfileContent::CONTENT_TYPE)
                                            }}
                                        />
                                    }
                                }}
                            </ForEach>
                            <Button
                                label="New settings profile"
                                glyph={ICON_ADD.to_owned()}
                                variant=ButtonVariant::Ghost
                                disabled={blocked}
                                @test_id={"settings.new-profile"}
                                on_click={move || {
                                    let count = adding
                                        .read(|settings| settings.root().profiles().len())
                                        .unwrap_or(0);
                                    let profile = editor.blocks().create_named(
                                        &SettingsProfileContent::default(),
                                        BlockParent::Block(editor.block_id()),
                                        format!("Profile {}", count + 1),
                                    );
                                    adding.operate(Settings::list_profile(profile));
                                }}
                            />
                        </List>
                    </Frame>
                </Scroll>
            </Frame>
        </Loaded>
    }
}

#[component]
pub fn SettingsProfileView(editor: Editor) -> NodeId {
    let profile = editor.block_content::<SettingsProfileContent>();
    let block = editor.block_id();
    let listed = editor.blocks().watch(BlockQuery::Block(block));
    let parent = create_memo(move || {
        listed
            .read()
            .into_iter()
            .next()
            .and_then(|info| match info.parent {
                BlockParent::Block(parent) => Some(parent),
                _ => None,
            })
    });
    let own = create_memo(move || {
        parent
            .get()
            .is_some_and(|parent| is_own_settings(&editor, parent))
    });
    let others = create_memo(move || !own.get());
    let loaded = profile_loaded(editor, profile, parent);
    let blocked = blocked(editor, profile, own);
    let client = editor.host().client_id();
    let used = create_memo(move || {
        parent.get().and_then(|parent| {
            editor
                .content_of::<SettingsContent>(parent)
                .read(|settings| settings.root().profile(client))
        }) == Some(Some(block))
    });
    let unused = create_memo(move || !used.get() && !blocked.get());
    let theme = use_theme();
    view! {
        <Loaded loaded={loaded}>
            <Frame color={theme.background}>
                <Scroll>
                    <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                        <List spacing=SPACING>
                            <Heading content="Settings profile" />
                            <Show condition={others}>
                                <Caption content=NOT_OURS />
                            </Show>
                            <Caption
                                content="What is set here replaces the settings for every device, on the devices that use this profile. What is not set follows them."
                            />
                            <Show condition={used}>
                                <Caption content="This device uses this profile." />
                            </Show>
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
                            <Kinds editor={editor} layer={profile} blocked={blocked} />
                        </List>
                    </Frame>
                </Scroll>
            </Frame>
        </Loaded>
    }
}

fn profile_loaded(
    editor: Editor,
    profile: ContentProjection<SettingsProfileContent>,
    parent: Memo<Option<Uuid>>,
) -> Memo<bool> {
    let content = profile.loaded();
    let blocks = editor.blocks();
    let listed = blocks.watch(BlockQuery::Block(editor.block_id()));
    let roots = blocks.watch(BlockQuery::Roots);
    let settings = editor.related_content::<SettingsContent>(parent).settled();
    create_memo(move || {
        content.get()
            && listed.is_loaded()
            && roots.is_loaded()
            && (parent.get().is_none() || settings.get())
    })
}

fn blocked<C: Layer>(editor: Editor, layer: ContentProjection<C>, own: Memo<bool>) -> Memo<bool> {
    let loaded = layer.project(|_| true);
    let read_only = editor.read_only();
    create_memo(move || !loaded.get() || read_only.get() || !own.get())
}

#[component]
fn Kinds<C: Layer>(editor: Editor, layer: ContentProjection<C>, blocked: Memo<bool>) -> NodeId {
    let open_ui = opener::<C, UiSettingsContent>(editor, layer);
    let open_input = opener::<C, InputSettingsContent>(editor, layer);
    let open_display = opener::<C, DisplaySettingsContent>(editor, layer);
    let open_editors = opener::<C, EditorSettingsContent>(editor, layer);
    let (ui_blocked, input_blocked) = (blocked, blocked);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Button
                label="UI settings"
                variant=ButtonVariant::Secondary
                disabled={ui_blocked}
                @test_id={"settings.ui-settings"}
                on_click={open_ui}
            />
            <Button
                label="Input settings"
                variant=ButtonVariant::Secondary
                disabled={input_blocked}
                @test_id={"settings.input-settings"}
                on_click={open_input}
            />
            <Button
                label="Display settings"
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={"settings.display-settings"}
                on_click={open_display}
            />
            <Button
                label="Default editors"
                variant=ButtonVariant::Secondary
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
) -> impl Fn() + Clone + use<C, K> {
    let resolved = layer.project(|layer| layer.kind(K::CONTENT_TYPE));
    let host = editor.host();
    move || {
        let id = resolved.get_untracked().unwrap_or_else(|| {
            let block = editor.create_child(&K::default());
            layer.operate(C::set_kind(K::CONTENT_TYPE, block));
            block
        });
        host.open_block(id, K::CONTENT_TYPE);
    }
}
