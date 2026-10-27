use beui::{Direction, Extent, ItemSize, Justify};
use block_editor_beui::be_block::settings::SettingsProfile;
use block_editor_beui::be_block::{
    BlockContent, DeviceClass, DisplaySettings, DisplaySettingsContent, Edit, SettingsContent,
    SettingsProfileContent,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{BlockQuery, ContentProjection, Editor};
use uuid::Uuid;

const CELL_PADDING_HORIZONTAL: Space = Space::Xl;
const CELL_PADDING_VERTICAL: Space = Space::Md;
const ACTION_SPACING: Space = Space::Md;
const HEADER_INSET: Space = Space::Xl3;
const HEADER_SPACING: Space = Space::Xs;

const SET_UP: &str = "Set up this device";
const SET_UP_CAPTION: &str = "This device starts with settings that suit its kind. Keep them here, or use the settings your other devices use.";

#[derive(Clone, Copy)]
pub(super) struct DeviceSetup {
    editor: Editor,
    profile: ContentProjection<SettingsProfileContent>,
    class: ReadSignal<Option<DeviceClass>>,
    own_block: Memo<Option<Uuid>>,
    every: ReadSignal<DisplaySettings>,
    own: ReadSignal<DisplaySettings>,
    rows: Memo<Vec<String>>,
}

impl DeviceSetup {
    pub(super) fn new(
        editor: Editor,
        profile: ContentProjection<SettingsProfileContent>,
        parent: Memo<Option<Uuid>>,
    ) -> Self {
        let class = profile.project(|profile| profile.root().class);
        let held =
            profile.project(|profile| profile.root().kind(DisplaySettingsContent::CONTENT_TYPE));
        let own_block = create_memo(move || held.get());
        let settings = editor.related_content::<SettingsContent>(parent);
        let shared =
            settings.project(|settings| settings.root().kind(DisplaySettingsContent::CONTENT_TYPE));
        let every_block = create_memo(move || shared.get());
        let every_content = editor.related_content::<DisplaySettingsContent>(every_block);
        let own_content = editor.related_content::<DisplaySettingsContent>(own_block);
        let every = every_content.project(|content| content.root());
        let own = own_content.project(|content| content.root());
        let rows = create_memo(move || {
            let Some(class) = class.get() else {
                return Vec::new();
            };
            own.with(|own| {
                every.with(|every| {
                    class
                        .display_defaults()
                        .iter()
                        .filter(|default| default.differs(own, every))
                        .map(|default| default.key().to_owned())
                        .collect()
                })
            })
        });
        Self {
            editor,
            profile,
            class,
            own_block,
            every,
            own,
            rows,
        }
    }

    pub(super) fn differs(&self) -> Memo<bool> {
        let rows = self.rows;
        create_memo(move || rows.with(|rows| !rows.is_empty()))
    }

    fn write(&self, edits: Vec<Edit>) {
        if edits.is_empty() {
            return;
        }
        match self.own_block.get_untracked() {
            Some(block) => {
                let content = self.editor.content_of::<DisplaySettingsContent>(block);
                for edit in edits {
                    content.operate(edit);
                }
            }
            None => {
                let mut content = DisplaySettingsContent::default();
                for edit in &edits {
                    content.apply(edit);
                }
                let block = self.editor.create_child(&content);
                self.profile.operate(SettingsProfile::set_kind(
                    DisplaySettingsContent::CONTENT_TYPE,
                    block,
                ));
            }
        }
    }

    fn choose(&self, class: DeviceClass) {
        let previous = self.class.get_untracked();
        if previous == Some(class) {
            return;
        }
        self.profile
            .operate(SettingsProfile::set_class(Some(class)));
        let edits = self
            .own
            .with_untracked(|own| class.display_edits(previous, own));
        self.write(edits);
    }

    fn use_usual(&self) {
        let Some(class) = self.class.get_untracked() else {
            return;
        };
        let rows = self.rows.get_untracked();
        let edits = self.own.with_untracked(|own| {
            class
                .display_defaults()
                .iter()
                .filter(|default| rows.iter().any(|key| key == default.key()))
                .map(|default| default.clear(own))
                .collect()
        });
        self.write(edits);
    }

    fn cell(&self, key: String, read: fn(&DeviceSetupCell<'_>) -> String) -> Memo<String> {
        let (class, own, every) = (self.class, self.own, self.every);
        create_memo(move || {
            let Some(class) = class.get() else {
                return String::new();
            };
            let defaults = class.display_defaults();
            let Some(default) = defaults.iter().find(|default| default.key() == key) else {
                return String::new();
            };
            own.with(|own| {
                every.with(|every| {
                    read(&DeviceSetupCell {
                        default,
                        own,
                        every,
                    })
                })
            })
        })
    }
}

struct DeviceSetupCell<'a> {
    default: &'a block_editor_beui::be_block::ClassDefault<DisplaySettings>,
    own: &'a DisplaySettings,
    every: &'a DisplaySettings,
}

#[component]
pub(super) fn SetUpHeading() -> NodeId {
    view! {
        <Frame padding={Padding::horizontal(HEADER_INSET)}>
            <List spacing=HEADER_SPACING>
                <Text style=TextStyle::HEADING>{SET_UP}</Text>
                <Text style=TextStyle::CAPTION>{SET_UP_CAPTION}</Text>
            </List>
        </Frame>
    }
}

#[component]
pub(super) fn DeviceSection(setup: DeviceSetup, blocked: Memo<bool>) -> NodeId {
    let editor = setup.editor;
    let block = editor.block_id();
    let listed = editor.blocks().watch(BlockQuery::Block(block));
    let name = create_memo(move || {
        listed
            .read()
            .ok()
            .and_then(|list| list.into_iter().next())
            .and_then(|info| info.name())
            .unwrap_or_default()
    });
    let class = setup.class;
    let selected = create_memo(move || {
        class
            .get()
            .and_then(|class| DeviceClass::ALL.iter().position(|known| *known == class))
    });
    view! {
        <SettingsSection title="Device">
            <SettingsRow label="Name" locked={blocked}>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={Binding::new(name, move |text: String| {
                        editor.blocks().set_name(block, Some(text));
                    })}
                    label="Name"
                    disabled={blocked}
                    @test_id={"setup.name"}
                />
            </SettingsRow>
            <SettingsRow
                label="Kind of device"
                description="Each kind starts with the settings that suit it, kept on this device."
                separated=true
                locked={blocked}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={DeviceClass::ALL.to_vec()}>|class: DeviceClass|
                            <ChoiceOption label={class.name().to_owned()} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected, move |index: Option<usize>| {
                        if let Some(class) = index.and_then(|index| DeviceClass::ALL.get(index)) {
                            setup.choose(*class);
                        }
                    })}
                    label="Kind of device"
                    placeholder="Choose"
                    disabled={blocked}
                    @test_id={"setup.kind"}
                />
            </SettingsRow>
        </SettingsSection>
    }
}

#[component]
pub(super) fn DifferencesSection(setup: DeviceSetup, blocked: Memo<bool>) -> NodeId {
    let rows = setup.rows;
    view! {
        <SettingsSection
            title="Where this device differs"
            description="Everything else follows your other devices."
        >
            <Table
                label="Where this device differs"
                heading="Setting"
                columns={vec![TableColumn::new("Your devices"), TableColumn::new("This device")]}
            >
                <ForEach keys={rows}>|key: String| {
                    let name = setup.cell(key.clone(), |cell| cell.default.name().to_owned());
                    let everywhere = setup.cell(key.clone(), |cell| cell.default.every_device(cell.every));
                    let here = setup.cell(key.clone(), |cell| cell.default.kept(cell.own).unwrap_or_default());
                    view! {
                        <TableRow label={name} @test_id={format!("setup.differs.{key}")}>
                            <TableCell text={everywhere} color=TextColor::Muted />
                            <TableCell text={here} />
                        </TableRow>
                    }
                }</ForEach>
            </Table>
            <Frame padding={Padding::horizontal(CELL_PADDING_HORIZONTAL)}>
                <Separator />
            </Frame>
            <Frame padding={Padding::symmetric(CELL_PADDING_HORIZONTAL, CELL_PADDING_VERTICAL)}>
                <List direction=Direction::Horizontal justify=Justify::End spacing=ACTION_SPACING>
                    <Button
                        label="Use my usual settings"
                        variant=ButtonVariant::Secondary
                        disabled={blocked}
                        @test_id={"setup.usual"}
                        on_click={move || setup.use_usual()}
                    />
                </List>
            </Frame>
        </SettingsSection>
    }
}

#[component]
pub(super) fn SetUpDone(on_done: ClickCallback) -> NodeId {
    view! {
        <Frame padding={Padding::horizontal(HEADER_INSET)}>
            <List direction=Direction::Horizontal justify=Justify::End spacing=ACTION_SPACING>
                <Button
                    label="Done"
                    variant=ButtonVariant::Primary
                    @test_id={"setup.done"}
                    on_click={on_done}
                />
            </List>
        </Frame>
    }
}
