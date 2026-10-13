use block_editor_beui::be_block::database::DatabaseValue;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ADD, ICON_SEARCH, ICON_TUNE};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Show, clone, component, create_memo,
    create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, IconButton, ModalSheet, Popover, Scroll,
    Separator, TextInput, ToggleButton, use_theme,
};
use block_editor_beui::narrow_chrome;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::app::display::DisplayOptions;
use crate::database_view::app::filters::{FilterBar, FilterList};

const GUTTER: f32 = 12.0;
const SPACING: f32 = 8.0;
const SEARCH_WIDTH: f32 = 220.0;
const DISPLAY_WIDTH: f32 = 380.0;

#[component]
pub fn ViewHeader(data: Data) -> NodeId {
    let narrow = narrow_chrome();
    let wide = create_memo(move || !narrow.get());
    let (wide_data, narrow_data) = (data.clone(), data.clone());
    let failed = create_memo(clone!(data -> move || data.error.with(Option::is_some)));
    let message = create_memo(clone!(data -> move || data.error.get().unwrap_or_default()));
    let dismiss = clone!(data -> move || data.dismiss_error());
    let theme = use_theme();
    view! {
        <List spacing=0.0>
            <Show condition={wide}>
                {move || clone!(wide_data -> view! {
                    <WideHeader data={wide_data} />
                })}
            </Show>
            <Show condition={narrow}>
                {move || clone!(narrow_data -> view! {
                    <NarrowHeader data={narrow_data} />
                })}
            </Show>
            <Show condition={failed}>
                {move || clone!(dismiss -> view! {
                    <Frame padding_horizontal=GUTTER padding_vertical=6.0>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Body
                                @sizing=ItemSize::Percent(100.0)
                                content={message}
                                color={theme.danger}
                                wrap=true
                            />
                            <Button
                                label="Dismiss"
                                variant=ButtonVariant::Secondary
                                @test_id={"database-view.dismiss-error"}
                                on_click={dismiss}
                            />
                        </List>
                    </Frame>
                })}
            </Show>
            <Separator />
        </List>
    }
}

#[component]
fn WideHeader(data: Data) -> NodeId {
    let count = row_count(&data);
    let (tabs, search, add, filters, display) = (
        data.clone(),
        data.clone(),
        data.clone(),
        data.clone(),
        data.clone(),
    );
    view! {
        <Frame padding_horizontal=GUTTER padding_top={Some(8.0)} padding_bottom={Some(6.0)}>
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Heading content={data.name} />
                    <ViewTabs @sizing={ItemSize::Percent(100.0).shrink(1.0)} data={tabs} />
                    <SearchField @sizing=ItemSize::Fixed(SEARCH_WIDTH) data={search} />
                    <NewRow data={add} compact=false />
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <FilterBar @sizing={ItemSize::Percent(100.0).shrink(1.0)} data={filters} />
                    <Caption content={count} />
                    <Popover
                        label="Display"
                        glyph={ICON_TUNE.to_owned()}
                        max_width=DISPLAY_WIDTH
                        @test_id={"database-view.display"}
                    >
                        {move |_| {
                            let data = display.clone();
                            view! {
                                <DisplayOptions data={data} />
                            }
                        }}
                    </Popover>
                </List>
            </List>
        </Frame>
    }
}

#[component]
fn NarrowHeader(data: Data) -> NodeId {
    let (sheet, set_sheet) = create_signal(false);
    let filtered = create_memo(clone!(data -> move || {
        match data.filter_items.with(Vec::len) {
            0 => "Filter and display".to_owned(),
            count => format!("Filter and display · {count}"),
        }
    }));
    let count = row_count(&data);
    let (tabs, search, add, sheet_data) = (data.clone(), data.clone(), data.clone(), data.clone());
    let open = move || set_sheet.set(true);
    let close = move || set_sheet.set(false);
    let done = close;
    view! {
        <Frame padding_horizontal=GUTTER padding_top={Some(8.0)} padding_bottom={Some(8.0)}>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Heading @sizing=ItemSize::Percent(100.0) content={data.name} />
                    <Caption content={count} />
                    <NewRow data={add} compact=true />
                </List>
                <Scroll direction=Direction::Horizontal>
                    <ViewTabs data={tabs} />
                </Scroll>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <SearchField @sizing=ItemSize::Percent(100.0) data={search} />
                    <Button
                        label={filtered}
                        glyph={ICON_TUNE.to_owned()}
                        variant=ButtonVariant::Secondary
                        @test_id={"database-view.filter-and-display"}
                        on_click={open}
                    />
                </List>
                <ModalSheet open={sheet} on_close={close}>
                    <Frame padding_horizontal={GUTTER * 1.5} padding_bottom={Some(GUTTER * 2.0)}>
                        <List spacing=GUTTER>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=SPACING
                            >
                                <Heading
                                    @sizing=ItemSize::Percent(100.0)
                                    content="Filter and display"
                                />
                                <Button
                                    label="Done"
                                    variant=ButtonVariant::Ghost
                                    @test_id={"database-view.sheet.done"}
                                    on_click={done}
                                />
                            </List>
                            <Caption content="Filters" />
                            <FilterList data={sheet_data.clone()} />
                            <Separator />
                            <DisplayOptions data={sheet_data} />
                        </List>
                    </Frame>
                </ModalSheet>
            </List>
        </Frame>
    }
}

fn row_count(data: &Data) -> block_editor_beui::beui::reactive::Memo<String> {
    let groups = data.groups;
    create_memo(move || {
        let count: usize = groups.with(|groups| groups.iter().map(|group| group.rows.len()).sum());
        match count {
            1 => "1 row".to_owned(),
            count => format!("{count} rows"),
        }
    })
}

#[component]
fn ViewTabs(data: Data) -> NodeId {
    let keys = create_memo(clone!(data -> move || {
        data.views.with(|views| views.iter().map(|(id, _)| *id).collect::<Vec<Uuid>>())
    }));
    let own = data.editor().block_id();
    let create = clone!(data -> move || data.new_view());
    let read_only = data.read_only;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <ForEach keys={keys}>
                {move |view: Uuid| {
                    let data = data.clone();
                    let name = create_memo(clone!(data -> move || {
                        data.views.with(|views| {
                            views.iter().find(|(id, _)| *id == view).map(|(_, name)| name.clone()).unwrap_or_default()
                        })
                    }));
                    let open = clone!(data -> move |_: bool| data.open_view(view));
                    view! {
                        <ToggleButton
                            label={name}
                            pressed={view == own}
                            @test_id={format!("database-view.view.{view}")}
                            on_change={open}
                        />
                    }
                }}
            </ForEach>
            <IconButton
                glyph={ICON_ADD.to_owned()}
                label="New view"
                variant=ButtonVariant::Ghost
                disabled={read_only}
                @test_id={"database-view.views.new"}
                on_click={create}
            />
        </List>
    }
}

#[component]
fn SearchField(data: Data) -> NodeId {
    let query = data.search;
    let typed = clone!(data -> move |text: String| data.set_search(text));
    view! {
        <TextInput
            value={query}
            placeholder="Search"
            glyph={ICON_SEARCH.to_owned()}
            clearable=true
            @test_id={"database-view.search"}
            on_change={typed}
        />
    }
}

#[component]
fn NewRow(data: Data, compact: bool) -> NodeId {
    let add = clone!(data -> move || {
        let mut values = Vec::new();
        if let Some(title) = data.title_field.get_untracked() {
            values.push((title, DatabaseValue::String(String::new())));
        }
        data.new_row(values);
    });
    let disabled = create_memo(clone!(data -> move || {
        data.read_only.get() || data.title_field.get().is_none()
    }));
    match compact {
        true => view! {
            <IconButton
                glyph={ICON_ADD.to_owned()}
                label="New"
                variant=ButtonVariant::Primary
                disabled={disabled}
                @test_id={"database-view.new"}
                on_click={add}
            />
        },
        false => view! {
            <Button
                label="New"
                glyph={ICON_ADD.to_owned()}
                variant=ButtonVariant::Primary
                disabled={disabled}
                @test_id={"database-view.new"}
                on_click={add}
            />
        },
    }
}
