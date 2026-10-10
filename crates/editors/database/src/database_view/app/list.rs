use block_editor_beui::be_block::BlockContent;
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::database::{DatabaseValue, short_id};
use block_editor_beui::be_block::database_schema::DatabaseFieldType;
use block_editor_beui::be_block::text::TextContent;
use block_editor_beui::beui::icons::{ICON_CLOSE, ICON_DELETE, ICON_NOTE_ADD};
use block_editor_beui::beui::reactive::{
    Align, Direction, Dynamic, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone,
    component, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, IconButton, ListRow, Scroll, Separator, TextInput,
    use_theme,
};
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::block_ui::database::{
    DatabaseBlockPickRequest, DatabaseValueChange, cell_text,
};
use block_editor_beui::database::DatabaseValueEditor;
use block_editor_beui::{ChildBlock, ChildMode, ChildTarget};

use crate::database_view::app::data::Data;
use crate::database_view::sort::display_rows;

const PADDING: f32 = 12.0;
const SPACING: f32 = 8.0;

#[component]
pub fn ListView(data: Data) -> NodeId {
    let keys = visible_rows(&data);
    let empty = create_memo(clone!(keys -> move || keys.with(Vec::is_empty)));
    let selected = selected_row(&data);
    let open = create_memo(clone!(selected -> move || selected.get().is_some()));
    let (lines, adder, detail) = (data.clone(), data.clone(), data);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                        <List spacing=2.0>
                            <QuickAdd data={adder} />
                            <Show condition={empty}>
                                <Caption content="Nothing to show yet." />
                            </Show>
                            <ForEach keys={keys}>
                                {move |row: ObjectId| {
                                    let data = lines.clone();
                                    view! {
                                        <ListLine data={data} row={row} />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                </Scroll>
                <Show condition={open}>
                    {move || clone!(detail selected -> view! {
                        <Frame @sizing=ItemSize::Percent(100.0)>
                            <List direction=Direction::Horizontal spacing=0.0>
                                <Separator direction=Direction::Vertical />
                                <RowDetail
                                    @sizing=ItemSize::Percent(100.0)
                                    data={detail}
                                    row={selected}
                                />
                            </List>
                        </Frame>
                    })}
                </Show>
            </List>
        </Frame>
    }
}

pub fn visible_rows(data: &Data) -> Memo<Vec<ObjectId>> {
    let data = data.clone();
    create_memo(move || {
        let ids = data.row_ids.get();
        let shown = data.rows.with(|rows| {
            data.fields.with(|fields| {
                data.labels.with(|labels| {
                    data.filters.with(|filters| {
                        display_rows(rows, None, data.sort.get(), filters, fields, labels)
                    })
                })
            })
        });
        shown
            .into_iter()
            .filter_map(|row| ids.get(row.index).copied())
            .collect()
    })
}

pub fn selected_row(data: &Data) -> Memo<Option<ObjectId>> {
    let data = data.clone();
    create_memo(move || {
        let selection = data.selected.get()?;
        data.row_ids.with(|ids| ids.get(selection.row).copied())
    })
}

#[component]
fn QuickAdd(data: Data) -> NodeId {
    let (typed, set_typed) = create_signal(String::new());
    let disabled = create_memo(clone!(data -> move || {
        data.read_only.get() || data.title_field.get().is_none()
    }));
    let edited = clone!(set_typed -> move |text: String| set_typed.set(text));
    let submitted = clone!(data -> move |text: String| {
        let title = text.trim().to_owned();
        let Some(field) = data.title_field.get_untracked() else {
            return;
        };
        if title.is_empty() {
            return;
        }
        data.add_row(vec![(field, DatabaseValue::String(title))]);
        set_typed.set(String::new());
    });
    view! {
        <Frame padding_vertical=4.0>
            <TextInput
                value={typed}
                placeholder="New item"
                disabled={disabled}
                @test_id={"database-view.list.new"}
                on_change={edited}
                on_submit={submitted}
            />
        </Frame>
    }
}

#[component]
fn ListLine(data: Data, row: ObjectId) -> NodeId {
    let index = create_memo(clone!(data -> move || {
        data.row_ids.with(|ids| ids.iter().position(|id| *id == row))
    }));
    let selected = create_memo(clone!(data index -> move || {
        let index = index.get();
        data.selected.get().is_some_and(|selection| Some(selection.row) == index)
    }));
    let code = create_memo(clone!(data -> move || short_id(&data.prefix.get(), row.as_uuid())));
    let title = create_memo(clone!(data index -> move || {
        let (Some(index), Some(field)) = (index.get(), data.title_field.get()) else {
            return String::new();
        };
        data.rows.with(|rows| match rows.get(index).and_then(|row| row.value(field)) {
            Some(DatabaseValue::String(text)) => text.clone(),
            _ => String::new(),
        })
    }));
    let summary = create_memo(clone!(data index -> move || {
        let Some(index) = index.get() else {
            return String::new();
        };
        let Some(entry) = data.rows.with(|rows| rows.get(index).cloned()) else {
            return String::new();
        };
        let title = data.title_field.get();
        data.fields.with(|fields| {
            data.labels.with(|labels| {
                fields
                    .iter()
                    .filter(|field| Some(field.id) != title)
                    .filter(|field| {
                        matches!(
                            field.field_type,
                            DatabaseFieldType::Enum | DatabaseFieldType::Relation
                        )
                    })
                    .map(|field| cell_text(&entry, field, labels))
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<String>>()
                    .join(" · ")
            })
        })
    }));
    let pick = clone!(data index -> move || {
        if let Some(index) = index.get_untracked() {
            data.select(index, data.title_field.get_untracked());
        }
    });
    view! {
        <ListRow
            selected={selected}
            @test_id={format!("database-view.list.row.{}", row.as_uuid())}
            on_click={pick.clone()}
            on_activate={pick}
        >
            <List spacing=2.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Caption content={code} align=TextAlign::Start />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Caption
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        content={summary}
                        align=TextAlign::End
                        ellipsis=true
                    />
                </List>
                <Body content={title} wrap=true />
            </List>
        </ListRow>
    }
}

#[component]
pub fn RowDetail(data: Data, row: Memo<Option<ObjectId>>) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={row}>
                {move |row: Option<ObjectId>| {
                    let data = data.clone();
                    match row {
                        Some(row) => view! {
                            <RowPage @sizing=ItemSize::Percent(100.0) data={data} row={row} />
                        },
                        None => view! {
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                        },
                    }
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn RowPage(data: Data, row: ObjectId) -> NodeId {
    let index = create_memo(clone!(data -> move || {
        data.row_ids.with(|ids| ids.iter().position(|id| *id == row))
    }));
    let code = create_memo(clone!(data -> move || short_id(&data.prefix.get(), row.as_uuid())));
    let values = create_memo(clone!(data index -> move || {
        let Some(index) = index.get() else {
            return Default::default();
        };
        data.rows.with(|rows| rows.get(index).map(|row| row.values().clone()).unwrap_or_default())
    }));
    let page = create_memo(clone!(data index -> move || {
        let index = index.get()?;
        data.rows.with(|rows| rows.get(index).and_then(|row| row.page()))
    }));
    let title = create_memo(clone!(data values -> move || {
        let field = data.title_field.get();
        values.with(|values| match field.and_then(|field| values.get(&field)) {
            Some(DatabaseValue::String(text)) => text.clone(),
            _ => String::new(),
        })
    }));
    let properties = create_memo(clone!(data -> move || {
        let title = data.title_field.get();
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| Some(field.id) != title)
                .cloned()
                .collect::<Vec<_>>()
        })
    }));
    let retitled = clone!(data page -> move |text: String| {
        let Some(field) = data.title_field.get_untracked() else {
            return;
        };
        data.set_value(row, field, Some(DatabaseValue::String(text.clone())));
        if let Some(page) = page.get_untracked() {
            data.editor().blocks().set_name(page, Some(text));
        }
    });
    let changed = clone!(data -> move |change: DatabaseValueChange| {
        data.set_value(row, change.field_id, change.value);
    });
    let picked = clone!(data index -> move |request: DatabaseBlockPickRequest| {
        if let Some(index) = index.get_untracked() {
            data.pick_value(index, request);
        }
    });
    let remove = clone!(data -> move || data.remove_row(row));
    let close = clone!(data -> move || data.deselect());
    let add_page = clone!(data title -> move || {
        data.create_page(row, title.get_untracked());
    });
    let missing = create_memo(clone!(page -> move || page.get().is_none()));
    let present = create_memo(clone!(page -> move || page.get().is_some()));
    let target = create_memo(clone!(page -> move || {
        page.get().map(|page| ChildTarget::new(page, TextContent::CONTENT_TYPE))
    }));
    let (delete_off, title_off, values_off, page_off) = (
        data.read_only.clone(),
        data.read_only.clone(),
        data.read_only.clone(),
        data.read_only.clone(),
    );
    let labels = data.labels.clone();
    let editor = data.editor().clone();
    view! {
        <List spacing=0.0>
            <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Caption content={code} @test_id={"database-view.detail.id"} />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <IconButton
                            glyph={ICON_DELETE.to_owned()}
                            label="Delete"
                            disabled={delete_off}
                            @test_id={"database-view.detail.delete"}
                            on_click={remove}
                        />
                        <IconButton
                            glyph={ICON_CLOSE.to_owned()}
                            label="Close"
                            @test_id={"database-view.detail.close"}
                            on_click={close}
                        />
                    </List>
                    <TextInput
                        value={title}
                        placeholder="Title"
                        disabled={title_off}
                        @test_id={"database-view.detail.title"}
                        on_change={retitled}
                    />
                    <DatabaseValueEditor
                        fields={properties}
                        values={values}
                        labels={labels}
                        disabled={values_off}
                        prefix="database-view.detail"
                        on_change={changed}
                        on_pick={picked}
                    />
                    <Show condition={missing}>
                        {move || clone!(add_page page_off -> view! {
                            <List direction=Direction::Horizontal spacing=0.0>
                                <Button
                                    glyph={ICON_NOTE_ADD.to_owned()}
                                    label="Add a page"
                                    variant=ButtonVariant::Secondary
                                    disabled={page_off}
                                    @test_id={"database-view.detail.add-page"}
                                    on_click={add_page}
                                />
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                            </List>
                        })}
                    </Show>
                </List>
            </Frame>
            <Show condition={present}>
                <ChildBlock
                    @sizing=ItemSize::Percent(100.0)
                    editor={editor.clone()}
                    block={target.clone()}
                    mode=ChildMode::Live
                    own_frame=true
                />
            </Show>
        </List>
    }
}
