use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_PLUS, ICON_SEARCH, ICON_SLIDERS_HORIZONTAL};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::narrow_chrome;
use database_types::database::DatabaseValue;
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
    let failed = create_memo(move || data.error.with(Option::is_some));
    let message = create_memo(move || data.error.get().unwrap_or_default());
    let dismiss = move || data.dismiss_error();
    view! {
        <List spacing=0.0>
            <Show condition={wide}>
                <WideHeader data={data} />
            </Show>
            <Show condition={narrow}>
                <NarrowHeader data={data} />
            </Show>
            <Show condition={failed}>
                <Frame padding={Sides::symmetric(GUTTER, 6.0)}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text @sizing=ItemSize::Percent(100.0) color=TextColor::Danger>
                            {message}
                        </Text>
                        <Button
                            label="Dismiss"
                            variant=ButtonVariant::Secondary
                            @test_id={"database-view.dismiss-error"}
                            on_click={dismiss}
                        />
                    </List>
                </Frame>
            </Show>
            <Separator />
        </List>
    }
}

#[component]
fn WideHeader(data: Data) -> NodeId {
    let count = row_count(data);
    view! {
        <Frame padding={Sides::horizontal(GUTTER).top(8.0).bottom(6.0)}>
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text style=TextStyle::HEADING>{data.name}</Text>
                    <ViewTabs @sizing={ItemSize::Percent(100.0).shrink(1.0)} data={data} />
                    <SearchField @sizing=ItemSize::Fixed(SEARCH_WIDTH) data={data} />
                    <NewRow data={data} compact=false />
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <FilterBar @sizing={ItemSize::Percent(100.0).shrink(1.0)} data={data} />
                    <Text style=TextStyle::CAPTION>{count}</Text>
                    <Popover
                        label="Display"
                        glyph=ICON_SLIDERS_HORIZONTAL
                        max_width=DISPLAY_WIDTH
                        @test_id={"database-view.display"}
                    >|_|
                        <DisplayOptions data={data} />
                    </Popover>
                </List>
            </List>
        </Frame>
    }
}

#[component]
fn NarrowHeader(data: Data) -> NodeId {
    let sheet = create_signal(false);
    let filtered = create_memo(move || match data.filter_items.with(Vec::len) {
        0 => "Filter and display".to_owned(),
        count => format!("Filter and display · {count}"),
    });
    let count = row_count(data);
    let open = move || sheet.set(true);
    let close = move || sheet.set(false);
    view! {
        <Frame padding={Sides::horizontal(GUTTER).top(8.0).bottom(8.0)}>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>
                        {data.name}
                    </Text>
                    <Text style=TextStyle::CAPTION>{count}</Text>
                    <NewRow data={data} compact=true />
                </List>
                <Scroll direction=Direction::Horizontal>
                    <ViewTabs data={data} />
                </Scroll>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <SearchField @sizing=ItemSize::Percent(100.0) data={data} />
                    <Button
                        label={filtered}
                        glyph=ICON_SLIDERS_HORIZONTAL
                        variant=ButtonVariant::Secondary
                        @test_id={"database-view.filter-and-display"}
                        on_click={open}
                    />
                </List>
                <Dialog
                    open={sheet}
                    title="Filter and display"
                    id="database-view.sheet"
                    on_dismiss={close}
                >
                    <Text style=TextStyle::CAPTION>"Filters"</Text>
                    <FilterList data={data} />
                    <Separator />
                    <DisplayOptions data={data} />
                </Dialog>
            </List>
        </Frame>
    }
}

fn row_count(data: Data) -> block_editor_beui::beui::reactive::Memo<String> {
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
    let keys = create_memo(move || {
        data.views
            .with(|views| views.iter().map(|(id, _)| *id).collect::<Vec<Uuid>>())
    });
    let own = data.editor().block_id();
    let create = move || data.new_view();
    let read_only = data.read_only;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <ForEach keys={keys}>|view: Uuid| {
                let name = create_memo(move || {
                    data.views.with(|views| {
                        views.iter().find(|(id, _)| *id == view).map(|(_, name)| name.clone()).unwrap_or_default()
                    })
                });
                let open = move |_: bool| data.open_view(view);
                view! {
                    <ToggleButton
                        label={name}
                        pressed={Binding::new(view == own, open)}
                        @test_id={format!("database-view.view.{view}")}
                    />
                }
            }</ForEach>
            <Button
                icon_only=true
                glyph=ICON_PLUS
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
    let typed = move |text: String| data.set_search(text);
    view! {
        <TextInput
            value={Binding::new(query, typed)}
            placeholder="Search"
            glyph=ICON_SEARCH
            clearable=true
            @test_id={"database-view.search"}
        />
    }
}

#[component]
fn NewRow(data: Data, compact: bool) -> NodeId {
    let add = move || {
        let mut values = Vec::new();
        if let Some(title) = data.title_field.get_untracked() {
            values.push((title, DatabaseValue::String(String::new())));
        }
        data.new_row(values);
    };
    let disabled = create_memo(move || data.read_only.get() || data.title_field.get().is_none());
    match compact {
        true => view! {
            <Button
                icon_only=true
                glyph=ICON_PLUS
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
                glyph=ICON_PLUS
                variant=ButtonVariant::Primary
                disabled={disabled}
                @test_id={"database-view.new"}
                on_click={add}
            />
        },
    }
}
