use beui::Overflow;
use block_editor_beui::be_block::{BlockContent, ObjectId};
use block_editor_beui::beui::icons::{
    ICON_ARROW_LEFT, ICON_CHEVRON_DOWN, ICON_CHEVRON_UP, ICON_ELLIPSIS, ICON_EXTERNAL_LINK,
    ICON_FILE_PLUS, ICON_TRASH, ICON_X,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{BackGesture, NodeId};
use block_editor_beui::block_ui::database::DatabaseValueChange;
use block_editor_beui::database::DatabaseBlockPickRequest;
use block_editor_beui::database::{ChoiceMark, DatabaseValueEditor};
use block_editor_beui::{ChildBlock, ChildMode, ChildTarget, narrow_chrome};
use database_types::database::{DatabaseValue, RowRef, short_id};
use database_types::database_schema::{DatabaseField, DatabaseFieldType};
use uuid::Uuid;

use crate::database_view::app::controls::{ChoicePicker, MARK, MARK_SIZE};
use crate::database_view::app::data::Data;
use crate::database_view::app::list::{iconic, row_title, visible_groups};
use beui::{Align, Direction, ItemSize};
use text_block_types::TextContent;

const PADDING: Space = Space::Xl2;
const SPACING: Space = Space::Md;
const LABEL_WIDTH: f32 = 104.0;

#[component]
pub fn RowPanel(data: Data, row: ObjectId) -> NodeId {
    let index = create_memo(move || {
        data.row_ids
            .with(|ids| ids.iter().position(|id| *id == row))
    });
    let code = create_memo(move || short_id(&data.prefix.get(), row.as_uuid()));
    let values = create_memo(move || {
        let Some(index) = index.get() else {
            return Default::default();
        };
        data.rows
            .with(|rows| rows.get(index).map(|row| row.cells()).unwrap_or_default())
    });
    let page = create_memo(move || {
        let index = index.get()?;
        data.rows
            .with(|rows| rows.get(index).and_then(|row| row.page()))
    });
    let title = create_memo(move || {
        let field = data.title_field.get();
        values.with(|values| match field.and_then(|field| values.get(&field)) {
            Some(DatabaseValue::String(text)) => text.clone(),
            _ => String::new(),
        })
    });
    let untitled = title.with_untracked(String::is_empty);
    let properties = create_memo(move || {
        let title = data.title_field.get();
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| Some(field.id) != title && !several(field))
                .map(|field| field.id)
                .collect::<Vec<Uuid>>()
        })
    });
    let link_fields = create_memo(move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| several(field))
                .map(|field| field.id)
                .collect::<Vec<Uuid>>()
        })
    });
    let groups = visible_groups(data);
    let order = create_memo(move || {
        let ids = data.row_ids.get();
        groups.with(|groups| {
            groups
                .iter()
                .flat_map(|group| group.rows.iter())
                .filter_map(|index| ids.get(*index).copied())
                .collect::<Vec<ObjectId>>()
        })
    });
    let step = move |by: isize| {
        order.with_untracked(|order| {
            let Some(at) = order.iter().position(|held| *held == row) else {
                return;
            };
            if let Some(next) = at.checked_add_signed(by).and_then(|at| order.get(at)) {
                data.select_row(*next);
            }
        });
    };
    let retitled = move |text: String| {
        let Some(field) = data.title_field.get_untracked() else {
            return;
        };
        data.set_value(row, field, Some(DatabaseValue::String(text.clone())));
        if let Some(page) = page.get_untracked() {
            data.editor().blocks().set_name(page, Some(text));
        }
    };
    let close = move || data.deselect();
    let open_page = move || {
        if let Some(index) = index.get_untracked() {
            data.open_page(index);
        }
    };
    let remove = move || data.remove_row(row);
    let add_page = move || {
        data.create_page(row, title.get_untracked());
    };
    let missing = create_memo(move || page.get().is_none());
    let present = create_memo(move || page.get().is_some());
    let target = create_memo(move || {
        page.get()
            .map(|page| ChildTarget::new(page, TextContent::CONTENT_TYPE))
    });
    let narrow = narrow_chrome();
    let wide = create_memo(move || !narrow.get());
    let (title_off, page_off, delete_off) = (data.read_only, data.read_only, data.read_only);
    let editor = data.editor();
    let row = view! {
        <Frame fill=ColorRole::Background>
            <List spacing=Space::None>
                <Frame padding={Padding::symmetric(SPACING, Space::Sm)}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs2>
                        <Show condition={narrow}>
                            <Button
                                icon_only=true
                                glyph=ICON_ARROW_LEFT
                                label="Back to the list"
                                variant=ButtonVariant::Ghost
                                @test_id={"database-view.detail.back"}
                                on_click={close}
                            />
                        </Show>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_UP
                            label="Previous row"
                            variant=ButtonVariant::Ghost
                            @test_id={"database-view.detail.previous"}
                            on_click={move || step(-1)}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_DOWN
                            label="Next row"
                            variant=ButtonVariant::Ghost
                            @test_id={"database-view.detail.next"}
                            on_click={move || step(1)}
                        />
                        <Frame padding={Padding::horizontal(Space::Sm)}>
                            <Text style=TextStyle::CAPTION @test_id={"database-view.detail.id"}>
                                {code}
                            </Text>
                        </Frame>
                        <Frame @sizing=ItemSize::Percent(100.0) />
                        <Button
                            icon_only=true
                            glyph=ICON_EXTERNAL_LINK
                            label="Open the page"
                            variant=ButtonVariant::Ghost
                            @test_id={"database-view.detail.open"}
                            on_click={open_page}
                        />
                        <MenuButton
                            icon_only=true
                            variant=ButtonVariant::Ghost
                            label="More"
                            glyph=ICON_ELLIPSIS
                            @test_id={"database-view.detail.more"}
                            items={view! {
                                <MenuItem
                                    label="Delete row"
                                    glyph=ICON_TRASH
                                    danger=true
                                    disabled={delete_off}
                                    @test_id={"database-view.detail.delete"}
                                    on_click={remove}
                                />
                            }}
                        />
                        <Show condition={wide}>
                            <Button
                                icon_only=true
                                glyph=ICON_X
                                label="Close"
                                variant=ButtonVariant::Ghost
                                @test_id={"database-view.detail.close"}
                                on_click={close}
                            />
                        </Show>
                    </List>
                </Frame>
                <Separator />
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame padding=PADDING>
                        <List spacing=Space::Xl>
                            <TextInput
                                value={Binding::new(title, retitled)}
                                placeholder="Title"
                                disabled={title_off}
                                focused={untitled}
                                @test_id={"database-view.detail.title"}
                            />
                            <List spacing=Space::Xs2>
                                <ForEach keys={properties}>|field: Uuid|
                                    <Property data={data} row={row} field={field} />
                                </ForEach>
                            </List>
                            <ForEach keys={link_fields}>|field: Uuid|
                                <LinkSection data={data} row={row} field={field} />
                            </ForEach>
                            <Separator />
                            <Show condition={missing}>
                                <List direction=Direction::Horizontal spacing=Space::None>
                                    <Button
                                        glyph=ICON_FILE_PLUS
                                        label="Write a description"
                                        variant=ButtonVariant::Ghost
                                        disabled={page_off}
                                        @test_id={"database-view.detail.add-page"}
                                        on_click={add_page}
                                    />
                                </List>
                            </Show>
                        </List>
                    </Frame>
                </Scroll>
                <Show condition={present}>
                    <ChildBlock
                        @sizing=ItemSize::Percent(100.0)
                        editor={editor}
                        block={target}
                        mode=ChildMode::Live
                        own_frame=true
                    />
                </Show>
            </List>
        </Frame>
    };
    on_back(true, move |gesture| {
        if gesture == BackGesture::Invoked {
            close();
        }
    });
    row
}

pub fn several(field: &DatabaseField) -> bool {
    field.field_type == DatabaseFieldType::Relation && field.relation_options.multiple
}

#[component]
fn Property(data: Data, row: ObjectId, field: Uuid) -> NodeId {
    let known = create_memo(move || {
        data.fields
            .with(|fields| fields.iter().find(|known| known.id == field).cloned())
    });
    let name = create_memo(move || {
        known.with(|known| {
            known
                .as_ref()
                .map(|known| known.name.clone())
                .unwrap_or_default()
        })
    });
    let choice = create_memo(move || {
        known.with(|known| {
            known
                .as_ref()
                .is_some_and(|known| known.field_type == DatabaseFieldType::Enum)
        })
    });
    let other = create_memo(move || !choice.get());
    let values = create_memo(move || {
        let index = data
            .row_ids
            .with(|ids| ids.iter().position(|id| *id == row));
        data.rows.with(|rows| {
            index
                .and_then(|index| rows.get(index))
                .map(|row| row.cells())
                .unwrap_or_default()
        })
    });
    let chosen = create_memo(move || {
        values.with(|values| match values.get(&field) {
            Some(DatabaseValue::Enum(option)) => Some(*option),
            _ => None,
        })
    });
    let picked = move |option: Option<Uuid>| {
        data.set_value(row, field, option.map(DatabaseValue::Enum));
    };
    let fields = create_memo(move || known.get().into_iter().collect::<Vec<_>>());
    let changed = move |change: DatabaseValueChange| {
        data.set_value(row, change.field_id, change.value);
    };
    let pick_block = move |request: DatabaseBlockPickRequest| {
        if let Some(index) = data.index_of(row) {
            data.pick_value(index, request);
        }
    };
    let labels = data.labels;
    let (choice_off, value_off) = (data.read_only, data.read_only);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Frame @sizing=ItemSize::Fixed(LABEL_WIDTH)>
                <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{name}</Text>
            </Frame>
            <Frame @sizing=ItemSize::Percent(100.0)>
                <List spacing=Space::None>
                    <Show condition={choice}>
                        <ChoicePicker
                            field={known}
                            chosen={chosen}
                            disabled={choice_off}
                            picker_id={format!("database-view.detail.field.{field}")}
                            on_pick={picked}
                        />
                    </Show>
                    <Show condition={other}>
                        <DatabaseValueEditor
                            fields={fields}
                            values={values}
                            labels={labels}
                            disabled={value_off}
                            prefix="database-view.detail"
                            headings=false
                            on_change={changed}
                            on_pick={pick_block}
                        />
                    </Show>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn LinkSection(data: Data, row: ObjectId, field: Uuid) -> NodeId {
    let known = create_memo(move || {
        data.fields
            .with(|fields| fields.iter().find(|known| known.id == field).cloned())
    });
    let links = create_memo(move || {
        let index = data
            .row_ids
            .with(|ids| ids.iter().position(|id| *id == row));
        data.rows.with(|rows| {
            match index
                .and_then(|index| rows.get(index))
                .and_then(|row| row.get(field))
            {
                Some(DatabaseValue::Links(links)) => links,
                Some(DatabaseValue::Relation(link)) => vec![link],
                _ => Vec::new(),
            }
        })
    });
    let heading = create_memo(move || {
        known.with(|known| {
            known
                .as_ref()
                .map(|known| known.name.clone())
                .unwrap_or_default()
        })
    });
    let count = create_memo(move || match links.with(Vec::len) {
        0 => String::new(),
        count => count.to_string(),
    });
    let addable = create_memo(move || {
        let Some(database) = known.with(|known| known.as_ref()?.relation_options.database) else {
            return Vec::new();
        };
        let held = links.get();
        let own = data.database_id.get().map(|database| RowRef {
            database,
            row: row.as_uuid(),
        });
        data.labels.with(|labels| {
            labels
                .rows_of(database)
                .into_iter()
                .filter(|(link, _)| !held.contains(link) && Some(*link) != own)
                .collect::<Vec<_>>()
        })
    });
    let add_keys = create_memo(move || {
        addable.with(|rows| rows.iter().map(|(link, _)| *link).collect::<Vec<RowRef>>())
    });
    let added = move |chosen: Option<RowRef>| {
        let Some(link) = chosen else {
            return;
        };
        let mut wanted = links.get_untracked();
        wanted.push(link);
        data.set_value(row, field, Some(DatabaseValue::Links(wanted)));
    };
    let add_off = data.read_only;
    view! {
        <List spacing=Space::Xs>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Text>{heading}</Text>
                <Text style=TextStyle::CAPTION>{count}</Text>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <Frame @sizing=ItemSize::Fixed(180.0)>
                    <Select
                        options={view! {
                            <ForEach keys={add_keys}>|link: RowRef| {
                                let label = create_memo(move || {
                                    addable.with(|rows| {
                                        rows.iter()
                                            .find(|(held, _)| *held == link)
                                            .map_or_else(String::new, |(_, label)| label.clone())
                                    })
                                });
                                view! {
                                    <ChoiceOption value=link label={label} />
                                }
                            }</ForEach>
                        }}
                        selected={Binding::new(None, added)}
                        label="Link a row"
                        placeholder="Link a row"
                        disabled={add_off}
                        @test_id={format!("database-view.detail.links.{field}.add")}
                    />
                </Frame>
            </List>
            <ForEach keys={links}>|link: RowRef|
                <LinkedRow data={data} row={row} field={field} link={link} links={links} />
            </ForEach>
        </List>
    }
}

#[component]
fn LinkedRow(
    data: Data,
    row: ObjectId,
    field: Uuid,
    link: RowRef,
    links: Memo<Vec<RowRef>>,
) -> NodeId {
    let local = create_memo(move || {
        let database = data.database_id.get()?;
        if link.database != database {
            return None;
        }
        data.index_of(ObjectId::from_uuid(link.row))
    });
    let option = create_memo(move || {
        let index = local.get()?;
        let entry = data.rows.with(|rows| rows.get(index).cloned())?;
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| iconic(field))
                .find_map(|field| match entry.get(field.id) {
                    Some(DatabaseValue::Enum(option)) => field.option(option).cloned(),
                    _ => None,
                })
        })
    });
    let marked = create_memo(move || option.with(Option::is_some));
    let code = create_memo(move || match local.get() {
        Some(_) => short_id(&data.prefix.get(), link.row),
        None => String::new(),
    });
    let label = create_memo(move || match local.get() {
        Some(index) => {
            let title = data.title_field.get();
            data.rows.with(|rows| row_title(rows.get(index), title))
        }
        None => data.labels.with(|labels| labels.row(&link)),
    });
    let open = move || {
        if local.get_untracked().is_some() {
            data.select_row(ObjectId::from_uuid(link.row));
        }
    };
    let remove = move || {
        let wanted: Vec<RowRef> = links
            .get_untracked()
            .into_iter()
            .filter(|held| *held != link)
            .collect();
        data.set_value(
            row,
            field,
            (!wanted.is_empty()).then_some(DatabaseValue::Links(wanted)),
        );
    };
    let read_only = data.read_only;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
            <ListRow
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("database-view.detail.links.{field}.{}", link.row)}
                on_click={open}
                on_activate={open}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Frame width={Size::Custom(MARK_SIZE)} height={Size::Custom(MARK_SIZE)}>
                        <Layers>
                            <Show condition={marked}>
                                <ChoiceMark option={option} size=MARK />
                            </Show>
                        </Layers>
                    </Frame>
                    <Text style=TextStyle::CAPTION>{code}</Text>
                    <Text
                        @sizing={ItemSize::Percent(100.0).shrink(1.0)}
                        overflow=Overflow::Ellipsis
                    >
                        {label}
                    </Text>
                </List>
            </ListRow>
            <Button
                icon_only=true
                glyph=ICON_X
                label="Remove link"
                variant=ButtonVariant::Ghost
                disabled={read_only}
                @test_id={format!("database-view.detail.links.{field}.{}.remove", link.row)}
                on_click={remove}
            />
        </List>
    }
}
