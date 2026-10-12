use block_editor_beui::be_block::database_schema::DatabaseField;
use block_editor_beui::beui::reactive::{
    Align, Callback, Child, Direction, ForEach, Frame, List, Memo, clone, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::context_menu::menu_style;
use block_editor_beui::beui::styled::theme::RADIUS;
use block_editor_beui::beui::styled::{Body, use_theme};
use block_editor_beui::beui::unstyled::{self, ButtonHandle, MenuButtonHandle, MenuItem};
use block_editor_beui::beui::NodeId;
use block_editor_beui::database::{ChoiceMark, choice_glyph};
use uuid::Uuid;

pub const MARK_SIZE: f32 = 16.0;

#[component]
pub fn ControlFace(handle: ButtonHandle, children: Child) -> NodeId {
    let theme = use_theme();
    let ButtonHandle {
        hovered,
        active,
        focused,
        ..
    } = handle;
    let fill = create_memo(clone!(theme -> move || {
        if active.get() {
            theme.pressed.get()
        } else if hovered.get() {
            theme.hover.get()
        } else {
            block_editor_beui::beui::Color32::TRANSPARENT
        }
    }));
    view! {
        <Frame
            color={fill}
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focused}
            radius=RADIUS
            padding_horizontal=8.0
            padding_vertical=5.0
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
    let option = create_memo(clone!(field chosen -> move || {
        let chosen = chosen.get()?;
        field.with(|field| field.as_ref()?.option(chosen).cloned())
    }));
    let label = create_memo(clone!(field option -> move || match option.get() {
        Some(option) => option.name,
        None => field.with(|field| {
            field
                .as_ref()
                .map_or_else(String::new, |field| format!("Set {}", field.name.to_lowercase()))
        }),
    }));
    let theme = use_theme();
    let color = create_memo(clone!(option theme -> move || match option.with(Option::is_some) {
        true => theme.text.get(),
        false => theme.text_muted.get(),
    }));
    let keys = create_memo(clone!(field -> move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.enum_options.iter().map(|option| option.id).collect())
                .unwrap_or_default()
        })
    }));
    let options = field.clone();
    let clear = on_pick.clone();
    let clear_id = format!("{picker_id}.clear");
    let option_id = picker_id.clone();
    let trigger_label = label.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::MenuButton
                label={trigger_label}
                disabled={disabled}
                menu={menu_style()}
                @test_id={picker_id.clone()}
                items={view! {
                    <ForEach keys={keys}>
                        {move |id: Uuid| {
                            let shown = create_memo(clone!(options -> move || {
                                options.with(|field| field.as_ref()?.option(id).cloned())
                            }));
                            let name = create_memo(clone!(shown -> move || {
                                shown.with(|shown| shown.as_ref().map(|shown| shown.name.clone()).unwrap_or_default())
                            }));
                            let glyph = create_memo(clone!(shown -> move || {
                                shown.with(|shown| shown.as_ref().map(|shown| choice_glyph(shown.icon)).unwrap_or_default()).to_owned()
                            }));
                            let pick = on_pick.clone();
                            view! {
                                <MenuItem
                                    label={name}
                                    glyph={glyph}
                                    row_test_id={format!("{option_id}.option.{id}")}
                                    on_click={move || pick.call(Some(id))}
                                />
                            }
                        }}
                    </ForEach>
                    <MenuItem
                        label="Clear"
                        separated=true
                        row_test_id={clear_id.clone()}
                        on_click={move || clear.call(None)}
                    />
                }}
                trigger={move |handle: MenuButtonHandle| {
                    let option = option.clone();
                    let label = label.clone();
                    let color = color.clone();
                    view! {
                        <ControlFace handle={handle.button}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <ChoiceMark option={option} size=MARK_SIZE />
                                <Body content={label} color={color} />
                            </List>
                        </ControlFace>
                    }
                }}
            />
        </List>
    }
}
