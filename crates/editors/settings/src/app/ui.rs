use block_editor_beui::be_block::settings::{Settings, SettingsProfile};
use block_editor_beui::be_block::{
    BlockContent, DisplaySettingsContent, EditorSettingsContent, InputSettingsContent, LiveEdit,
    SettingsContent, SettingsProfileContent, UiSettingsContent,
};
use block_editor_beui::beui::icons::{
    ICON_ADD, ICON_APPS, ICON_DEVICES, ICON_KEYBOARD, ICON_MONITOR, ICON_PALETTE,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::root_settings::is_own_settings;
use block_editor_beui::{BlockParent, BlockQuery, ContentProjection, Editor, Loaded};
use std::rc::Rc;
use uuid::Uuid;

const PADDING: f32 = 16.0;
const SPACING: f32 = 10.0;
const SECTION_SPACING: f32 = 24.0;
const CAPTION_SPACING: f32 = 2.0;
const ROW_SPACING: f32 = 2.0;
const ROW_PADDING: f32 = 4.0;
const ROW_GAP: f32 = 12.0;
const CARD_PADDING: f32 = 6.0;
const NOTICE_INSET: f32 = 24.0;
const MARKER_WIDTH: f32 = 3.0;
const MARKER_GAP: f32 = 6.0;
const MARKER_HEIGHT: f32 = 28.0;
const DIALOG_SPACING: f32 = 16.0;
const BUTTON_SPACING: f32 = 8.0;

const NOT_OURS: &str = "These settings belong to another account. This device never uses them, and they stay as they are.";

type Reset = (&'static str, Rc<dyn Fn()>);

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
    let shown = shown_block(editor);
    view! {
        <Loaded loaded={loaded}>
            <Frame fill=ColorRole::Background>
                <Scroll>
                    <Frame padding=PADDING>
                        <List spacing=SECTION_SPACING>
                            <Show condition={others}>
                                <Text style=TextStyle::CAPTION string=NOT_OURS wrap=true />
                            </Show>
                            <Section
                                title="Every device"
                                caption="A settings profile can change any of these for the devices that use it."
                            >
                                <Kinds
                                    editor={editor}
                                    layer={settings}
                                    blocked={blocked}
                                    shown={shown}
                                />
                            </Section>
                            <Section
                                title="Settings profiles"
                                caption="Each device uses one, and starts with one of its own."
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
                                                glyph={ICON_DEVICES.to_owned()}
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
                            </Section>
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
            .and_then(|info| info.parent?.block())
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
    let shown = shown_block(editor);
    let status = create_memo(move || match used.get() {
        true => "Used on this device".to_owned(),
        false => "Not used on this device".to_owned(),
    });
    view! {
        <Loaded loaded={loaded}>
            <SettingsPage>
                <Show condition={others}>
                    <Frame padding={Sides::horizontal(NOTICE_INSET)}>
                        <Text style=TextStyle::CAPTION string=NOT_OURS wrap=true />
                    </Frame>
                </Show>
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
                <SettingsSection title="In this profile">
                    <Frame padding=CARD_PADDING>
                        <Kinds editor={editor} layer={profile} blocked={blocked} shown={shown} />
                    </Frame>
                </SettingsSection>
            </SettingsPage>
        </Loaded>
    }
}

#[component]
fn Section(title: &'static str, caption: &'static str, children: Child) -> NodeId {
    let captioned = !caption.is_empty();
    view! {
        <List spacing=SPACING>
            <List spacing=CAPTION_SPACING>
                <Text style=TextStyle::HEADING string=title />
                <Show condition={captioned}>
                    <Text style=TextStyle::CAPTION string=caption wrap=true />
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
    glyph: Prop<String>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] modified: Prop<bool>,
    #[prop(default = String::new())] reset_test_id: Prop<String>,
    on_reset: ClickCallback,
    on_click: ClickCallback,
) -> NodeId {
    let label = create_memo(move || label.get());
    let detail = create_memo(move || detail.get());
    let glyph = create_memo(move || glyph.get());
    let off = create_memo(move || disabled.get());
    let modified = create_memo(move || modified.get());
    let reset_test_id = create_memo(move || reset_test_id.get());
    let has_detail = create_memo(move || !detail.get().is_empty());
    let resets = !on_reset.is_empty();
    let resettable = create_memo(move || resets && modified.get() && !off.get());
    let marker = create_memo(move || match modified.get() {
        true => ColorRole::Accent,
        false => ColorRole::None,
    });
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
            <Frame padding={Sides::vertical(ROW_PADDING)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_GAP>
                    <List direction=Direction::Horizontal align=Align::Center spacing=MARKER_GAP>
                        <Frame
                            width=MARKER_WIDTH
                            height=MARKER_HEIGHT
                            fill={marker}
                            radius=Radius::Full
                        />
                        <Text color={icon_color} align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </List>
                    <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                        <Text string={label} color={text_color} ellipsis=true />
                        <Show condition={has_detail}>
                            <Text style=TextStyle::CAPTION string={detail} ellipsis=true />
                        </Show>
                    </List>
                    <Show condition={resettable}>
                        <Link
                            label="Reset"
                            size=TextSize::Xs
                            @test_id={reset_test_id}
                            on_click={on_reset}
                        />
                    </Show>
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
    let (open_ui, ui, reset_ui) = opener::<C, UiSettingsContent>(editor, layer);
    let (open_input, input, reset_input) = opener::<C, InputSettingsContent>(editor, layer);
    let (open_display, display, reset_display) = opener::<C, DisplaySettingsContent>(editor, layer);
    let (open_editors, editors, reset_editors) = opener::<C, EditorSettingsContent>(editor, layer);
    let set = move |block: ReadSignal<Option<Uuid>>| create_memo(move || block.get().is_some());
    let showing = move |block: ReadSignal<Option<Uuid>>| {
        create_memo(move || block.get().is_some_and(|block| shown.get() == Some(block)))
    };
    let resets: Rc<[Reset]> = Rc::new([
        ("appearance", Rc::new(reset_ui) as Rc<dyn Fn()>),
        ("input", Rc::new(reset_input)),
        ("display", Rc::new(reset_display)),
        ("default editor", Rc::new(reset_editors)),
    ]);
    let (resetting, set_resetting) = create_signal(None::<usize>);
    let asking = create_memo(move || resetting.get().is_some());
    let named = resets.clone();
    let explained = create_memo(move || {
        let name = resetting.get().map_or("", |index| named[index].0);
        format!(
            "This deletes the block that holds these {name} settings, and everything set in it. They go back to what is set beneath this."
        )
    });
    let confirmed = resets.clone();
    let confirm = move || {
        if let Some(index) = resetting.get_untracked() {
            set_resetting.set(None);
            (confirmed[index].1)();
        }
    };
    let ask = move |index: usize| move || set_resetting.set(Some(index));
    view! {
        <List spacing=ROW_SPACING>
            <SettingRow
                label="Appearance"
                detail="Theme"
                glyph={ICON_PALETTE.to_owned()}
                selected={showing(ui)}
                disabled={blocked}
                modified={set(ui)}
                reset_test_id="settings.ui-settings.reset"
                on_reset={ask(0)}
                @test_id={"settings.ui-settings"}
                on_click={open_ui}
            />
            <SettingRow
                label="Input"
                detail="Keyboard, pointers and pasting"
                glyph={ICON_KEYBOARD.to_owned()}
                selected={showing(input)}
                disabled={blocked}
                modified={set(input)}
                reset_test_id="settings.input-settings.reset"
                on_reset={ask(1)}
                @test_id={"settings.input-settings"}
                on_click={open_input}
            />
            <SettingRow
                label="Displays"
                detail="Resolution, sleep and lock"
                glyph={ICON_MONITOR.to_owned()}
                selected={showing(display)}
                disabled={blocked}
                modified={set(display)}
                reset_test_id="settings.display-settings.reset"
                on_reset={ask(2)}
                @test_id={"settings.display-settings"}
                on_click={open_display}
            />
            <SettingRow
                label="Default editors"
                detail="Which editor opens each type"
                glyph={ICON_APPS.to_owned()}
                selected={showing(editors)}
                disabled={blocked}
                modified={set(editors)}
                reset_test_id="settings.default-editors.reset"
                on_reset={ask(3)}
                @test_id={"settings.default-editors"}
                on_click={open_editors}
            />
            <Dialog
                open={asking}
                title="Reset these settings?"
                on_dismiss={move || set_resetting.set(None)}
            >
                <List spacing=DIALOG_SPACING>
                    <Text style=TextStyle::PARAGRAPH string={explained} wrap=true />
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        justify=Justify::End
                        spacing=BUTTON_SPACING
                    >
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            @test_id={"settings.reset.cancel"}
                            on_click={move || set_resetting.set(None)}
                        />
                        <Button
                            label="Reset"
                            variant=ButtonVariant::Primary
                            @test_id={"settings.reset.confirm"}
                            on_click={confirm}
                        />
                    </List>
                </List>
            </Dialog>
        </List>
    }
}

fn opener<C: Layer, K: BlockContent + Default>(
    editor: Editor,
    layer: ContentProjection<C>,
) -> (
    impl Fn() + Clone + use<C, K>,
    ReadSignal<Option<Uuid>>,
    impl Fn() + Clone + use<C, K>,
) {
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
    let deleting = editor.host();
    let reset = move || {
        if let Some(id) = resolved.get_untracked() {
            deleting.delete_block(
                id,
                K::CONTENT_TYPE,
                BlockParent::Block(editor.block_id()),
                false,
            );
        }
    };
    (open, resolved, reset)
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
