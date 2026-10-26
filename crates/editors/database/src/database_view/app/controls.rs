use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::database::{ChoiceMark, choice_glyph};
use database_types::database_schema::DatabaseField;
use uuid::Uuid;

pub const MARK: TextSize = TextSize::Sm;
pub const MARK_SIZE: f32 = MARK.icon_px();

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
        <List direction=Direction::Horizontal spacing=Space::None>
            <MenuButton
                label={label}
                disabled={disabled}
                variant=ButtonVariant::Ghost
                size=ButtonSize::Compact
                arrow=false
                leading={view! { <ChoiceMark option={option} size=MARK /> }}
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
            />
        </List>
    }
}
