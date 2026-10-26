use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::context_menu::menu_style;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, ButtonHandle, MenuItem, PopupTriggerHandle};
use block_editor_beui::database::{ChoiceMark, choice_glyph};
use database_types::database_schema::DatabaseField;
use uuid::Uuid;

pub const MARK: TextSize = TextSize::Sm;
pub const MARK_SIZE: f32 = MARK.icon_px();

#[component]
pub fn ControlFace(handle: ButtonHandle, children: Child) -> NodeId {
    let theme = use_theme();
    let ButtonHandle {
        hovered,
        active,
        focused,
        ..
    } = handle;
    let fill = create_memo(move || {
        if active.get() {
            theme.pressed.get()
        } else if hovered.get() {
            theme.hover.get()
        } else {
            block_editor_beui::beui::Color32::TRANSPARENT
        }
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            border={create_memo(move || focused.get().then_some(Border::new(ColorRole::Accent).width(2.0)))}
            radius=Radius::Md
            padding={Sides::symmetric(8.0, 5.0)}
        >
            {children}
        </Frame>
    }
}

#[component]
pub fn ChoicePicker(
    field: Memo<Option<DatabaseField>>,
    chosen: Memo<Option<Uuid>>,
    disabled: Memo<bool>,
    picker_id: String,
    on_pick: Callback<Option<Uuid>>,
) -> NodeId {
    let option = create_memo(move || {
        let chosen = chosen.get()?;
        field.with(|field| field.as_ref()?.option(chosen).cloned())
    });
    let label = create_memo(move || match option.get() {
        Some(option) => option.name,
        None => field.with(|field| {
            field.as_ref().map_or_else(String::new, |field| {
                format!("Set {}", field.name.to_lowercase())
            })
        }),
    });
    let color = create_memo(move || match option.with(Option::is_some) {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let keys = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| {
                    field
                        .enum_options
                        .iter()
                        .map(|option| option.id)
                        .collect::<Vec<Uuid>>()
                })
                .unwrap_or_default()
        })
    });
    let clear_id = format!("{picker_id}.clear");
    let option_id = picker_id.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::MenuButton
                label={label}
                disabled={disabled}
                menu={menu_style()}
                @test_id={picker_id.clone()}
                items={view! {
                    <ForEach keys={keys}>|id: Uuid| {
                        let shown = create_memo(move || {
                            field.with(|field| field.as_ref()?.option(id).cloned())
                        });
                        let name = create_memo(move || {
                            shown.with(|shown| shown.as_ref().map(|shown| shown.name.clone()).unwrap_or_default())
                        });
                        let glyph = create_memo(move || {
                            shown.with(|shown| shown.as_ref().map(|shown| choice_glyph(shown.icon)).unwrap_or_default()).to_owned()
                        });
                        view! {
                            <MenuItem
                                label={name}
                                glyph={glyph}
                                row_test_id={format!("{option_id}.option.{id}")}
                                on_click={move || on_pick.call(Some(id))}
                            />
                        }
                    }</ForEach>
                    <MenuItem
                        label="Clear"
                        separated=true
                        row_test_id={clear_id.clone()}
                        on_click={move || on_pick.call(None)}
                    />
                }}
                trigger={move |handle: PopupTriggerHandle| {
                    view! {
                        <ControlFace handle={handle.button}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <ChoiceMark option={option} size=MARK />
                                <Text color={color}>{label}</Text>
                            </List>
                        </ControlFace>
                    }
                }}
            />
        </List>
    }
}
