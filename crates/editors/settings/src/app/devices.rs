use std::collections::BTreeSet;
use std::rc::Rc;

use block_editor_beui::be_block::display_settings::Rotation;
use block_editor_beui::be_block::setting::Setting;
use block_editor_beui::be_block::{
    BlockContent, DeviceClass, DisplaySettings, DisplaySettingsContent, InputSettings,
    InputSettingsContent, SettingsContent, SettingsProfileContent,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{BlockQuery, ContentProjection, Editor};
use uuid::Uuid;

use crate::display::modes::{lock_after_label, rate_label, screen_off_label, size_label};

const SAME: &str = "same";
const NOT_SET: &str = "not set";
const KEPT_SINCE_SETUP: &str = "kept since setup";
const KEPT_ON_DEVICE: &str = "kept on device";
const NOTHING_DIFFERS: &str = "Every profile follows the settings for all devices.";

#[derive(Clone, Default, PartialEq)]
struct Values {
    class: Option<DeviceClass>,
    display: DisplaySettings,
    input: InputSettings,
}

type Describe = Rc<dyn Fn(&Values) -> Option<String>>;
type SinceSetup = Rc<dyn Fn(&Values) -> bool>;

struct Described {
    key: String,
    name: String,
    describe: Describe,
    since_setup: SinceSetup,
}

fn display<V: Clone + 'static>(
    setting: Setting<DisplaySettings, V>,
    describe: impl Fn(&V) -> String + 'static,
) -> Described {
    let key = setting.key().to_owned();
    let name = setting.name().to_owned();
    let read = setting.clone();
    let defaulted = key.clone();
    Described {
        key,
        name,
        describe: Rc::new(move |values| read.get(&values.display).map(|value| describe(&value))),
        since_setup: Rc::new(move |values| {
            values.class.is_some_and(|class| {
                class
                    .display_defaults()
                    .iter()
                    .any(|default| default.key() == defaulted && default.holds(&values.display))
            })
        }),
    }
}

fn input<V: Clone + 'static>(
    setting: Setting<InputSettings, V>,
    describe: impl Fn(&V) -> String + 'static,
) -> Described {
    Described {
        key: setting.key().to_owned(),
        name: setting.name().to_owned(),
        describe: Rc::new(move |values| setting.get(&values.input).map(|value| describe(&value))),
        since_setup: Rc::new(|_| false),
    }
}

fn on_off(on: &bool) -> String {
    match on {
        true => "On".to_owned(),
        false => "Off".to_owned(),
    }
}

fn text(value: &str) -> String {
    match value.is_empty() {
        true => "None".to_owned(),
        false => value.to_owned(),
    }
}

fn screen_name(id: &str) -> String {
    let mut parts = id.split('|');
    match (parts.next(), parts.next()) {
        (Some(make), Some(model)) if !model.is_empty() => format!("{make} {model}"),
        _ => id.to_owned(),
    }
}

fn rotation(rotation: &Rotation) -> String {
    match rotation {
        Rotation::Normal => "Upright".to_owned(),
        turned => format!("Turned {}°", turned.degrees()),
    }
}

fn described(screens: &BTreeSet<String>) -> Vec<Described> {
    let mut rows = vec![
        display(DisplaySettings::theme_setting(), |theme| {
            theme.name().to_owned()
        }),
        display(DisplaySettings::scale_setting(), |percent| {
            format!("{percent}%")
        }),
        display(DisplaySettings::keyboard_setting(), |percent| {
            format!("{percent}%")
        }),
        display(DisplaySettings::rotate_in_bar_setting(), on_off),
        display(DisplaySettings::screen_off_setting(), |off| {
            screen_off_label(*off)
        }),
        display(DisplaySettings::lock_after_setting(), |lock| {
            lock_after_label(*lock)
        }),
    ];
    for id in screens {
        let screen = screen_name(id);
        let mut mode = display(DisplaySettings::mode_setting(id), |mode| {
            format!(
                "{} at {}",
                size_label((mode.width, mode.height)),
                rate_label(mode.refresh_millihertz)
            )
        });
        mode.name = format!("{}, {screen}", mode.name);
        let mut turned = display(DisplaySettings::rotation_setting(id), rotation);
        turned.name = format!("{}, {screen}", turned.name);
        rows.extend([mode, turned]);
    }
    rows.extend([
        input(InputSettings::layout_setting(), |value: &String| {
            text(value)
        }),
        input(InputSettings::variant_setting(), |value: &String| {
            text(value)
        }),
        input(InputSettings::options_setting(), |value: &String| {
            text(value)
        }),
        input(InputSettings::repeat_delay_setting(), |delay| {
            format!("{delay} ms")
        }),
        input(InputSettings::repeat_rate_setting(), |rate| {
            format!("{rate} per second")
        }),
        input(InputSettings::middle_click_paste_setting(), on_off),
    ]);
    rows
}

#[derive(Clone, PartialEq)]
struct Profile {
    id: Uuid,
    name: String,
    values: Values,
}

#[derive(Clone, Default, PartialEq)]
struct Cell {
    text: String,
    caption: String,
    kept: bool,
}

#[derive(Clone, PartialEq)]
struct Row {
    key: String,
    name: String,
    every: String,
    cells: Vec<(Uuid, Cell)>,
}

#[derive(Clone, Default, PartialEq)]
struct Model {
    columns: Vec<TableColumn>,
    profiles: Vec<Uuid>,
    rows: Vec<Row>,
}

impl Model {
    fn build(every: &Values, profiles: &[Profile]) -> Self {
        let screens: BTreeSet<String> = std::iter::once(every)
            .chain(profiles.iter().map(|profile| &profile.values))
            .flat_map(|values| values.display.screens.keys().cloned())
            .collect();
        let rows = described(&screens)
            .into_iter()
            .filter_map(|row| {
                let cells: Vec<(Uuid, Cell)> = profiles
                    .iter()
                    .map(|profile| {
                        let cell = match (row.describe)(&profile.values) {
                            Some(text) => Cell {
                                text,
                                caption: match (row.since_setup)(&profile.values) {
                                    true => KEPT_SINCE_SETUP.to_owned(),
                                    false => KEPT_ON_DEVICE.to_owned(),
                                },
                                kept: true,
                            },
                            None => Cell {
                                text: SAME.to_owned(),
                                ..Cell::default()
                            },
                        };
                        (profile.id, cell)
                    })
                    .collect();
                cells.iter().any(|(_, cell)| cell.kept).then(|| Row {
                    every: (row.describe)(every).unwrap_or_else(|| NOT_SET.to_owned()),
                    key: row.key,
                    name: row.name,
                    cells,
                })
            })
            .collect();
        let columns = std::iter::once(TableColumn::new("All devices"))
            .chain(profiles.iter().map(|profile| {
                let column = TableColumn::new(profile.name.clone());
                match profile.values.class {
                    Some(class) => column.caption(class.name()),
                    None => column,
                }
            }))
            .collect();
        Self {
            columns,
            profiles: profiles.iter().map(|profile| profile.id).collect(),
            rows,
        }
    }
}

fn values(editor: Editor, kinds: impl Fn(uuid::Uuid) -> Option<Uuid>) -> Values {
    let display = kinds(DisplaySettingsContent::CONTENT_TYPE)
        .and_then(|block| {
            editor
                .content_of::<DisplaySettingsContent>(block)
                .read(|content| content.root())
                .ok()
        })
        .unwrap_or_default();
    let input = kinds(InputSettingsContent::CONTENT_TYPE)
        .and_then(|block| {
            editor
                .content_of::<InputSettingsContent>(block)
                .read(|content| content.root())
                .ok()
        })
        .unwrap_or_default();
    Values {
        class: None,
        display,
        input,
    }
}

#[component]
pub(super) fn DevicesTable(editor: Editor, settings: ContentProjection<SettingsContent>) -> NodeId {
    let children = editor.watch_blocks(BlockQuery::Children(editor.block_id()));
    let model = create_memo(move || {
        let Ok(root) = settings.read(|settings| settings.root()) else {
            return Model::default();
        };
        let every = values(editor, |kind| root.kind(kind));
        let mut profiles: Vec<Profile> = root
            .profiles()
            .into_iter()
            .map(|id| {
                let profile = editor
                    .content_of::<SettingsProfileContent>(id)
                    .read(|content| content.root())
                    .unwrap_or_default();
                let name = children
                    .get()
                    .into_iter()
                    .flatten()
                    .find(|info| info.id == id)
                    .and_then(|info| info.name())
                    .unwrap_or_else(|| "Profile".to_owned());
                Profile {
                    id,
                    name,
                    values: Values {
                        class: profile.class,
                        ..values(editor, |kind| profile.kind(kind))
                    },
                }
            })
            .collect();
        profiles.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Model::build(&every, &profiles)
    });
    let columns = create_memo(move || model.with(|model| model.columns.clone()));
    let keys = create_memo(move || {
        model.with(|model| {
            model
                .rows
                .iter()
                .map(|row| row.key.clone())
                .collect::<Vec<_>>()
        })
    });
    let differs = create_memo(move || !keys.with(Vec::is_empty));
    let same = create_memo(move || !differs.get());
    view! {
        <List spacing=0.0>
            <Show condition={same}>
                <Text style=TextStyle::CAPTION @test_id={"devices.same"}>{NOTHING_DIFFERS}</Text>
            </Show>
            <Show condition={differs}>
                <Frame fill=ColorRole::Surface border={Border::DEFAULT} radius=Radius::Lg>
                    <Table label="Devices and profiles" heading="Setting" columns={columns}>
                        <ForEach keys={keys}>|key: String|
                            <DevicesRow model={model} key={key} />
                        </ForEach>
                    </Table>
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn DevicesRow(model: Memo<Model>, key: String) -> NodeId {
    let row = {
        let key = key.clone();
        create_memo(move || {
            model.with(|model| model.rows.iter().find(|row| row.key == key).cloned())
        })
    };
    let name = create_memo(move || {
        row.with(|row| row.as_ref().map(|row| row.name.clone()))
            .unwrap_or_default()
    });
    let every = create_memo(move || {
        row.with(|row| row.as_ref().map(|row| row.every.clone()))
            .unwrap_or_default()
    });
    let profiles = create_memo(move || model.with(|model| model.profiles.clone()));
    let every_color = create_memo(move || match every.with(|every| every == NOT_SET) {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let id = format!("devices.{key}");
    let (row_id, all_id) = (id.clone(), format!("{id}.all"));
    view! {
        <TableRow label={name} @test_id={row_id}>
            <TableCell text={every} color={every_color} @test_id={all_id} />
            <ForEach keys={profiles}>|profile: Uuid| {
                let cell = create_memo(move || {
                    row.with(|row| {
                        row.as_ref()
                            .and_then(|row| row.cells.iter().find(|(id, _)| *id == profile))
                            .map(|(_, cell)| cell.clone())
                            .unwrap_or_default()
                    })
                });
                let text = create_memo(move || cell.with(|cell| cell.text.clone()));
                let caption = create_memo(move || cell.with(|cell| cell.caption.clone()));
                let color = create_memo(move || match cell.with(|cell| cell.kept) {
                    true => TextColor::Default,
                    false => TextColor::Muted,
                });
                view! {
                    <TableCell
                        text={text}
                        caption={caption}
                        color={color}
                        @test_id={format!("{id}.{profile}")}
                    />
                }
            }</ForEach>
        </TableRow>
    }
}
