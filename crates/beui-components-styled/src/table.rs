use crate::border::Separator;
use crate::frame::Frame;
use crate::list::List;
use crate::text::{Text, TextColor, TextStyle};
use crate::tokens::{Padding, Space};
use beui_components_unstyled as unstyled;
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;

const LABEL_WEIGHT: f32 = 4.0;
const CELL_WEIGHT: f32 = 3.0;
const CELL_PADDING_HORIZONTAL: Space = Space::Xl;
const CELL_PADDING: Padding = Padding::symmetric(CELL_PADDING_HORIZONTAL, Space::Md);
const CELL_SPACING: Space = Space::Lg;
const CAPTION_SPACING: Space = Space::Xs2;
const COLUMN_TITLE: TextStyle = TextStyle::CAPTION.bold();

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableColumn {
    pub title: String,
    pub caption: String,
}

impl TableColumn {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            caption: String::new(),
        }
    }

    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }
}

#[component]
pub fn Table(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] heading: Prop<String>,
    columns: Prop<Vec<TableColumn>>,
    children: Children<ListItem>,
) -> NodeId {
    let heading = create_memo(move || TableColumn::new(heading.get()));
    let columns = create_memo(move || columns.get());
    let indices = create_memo(move || (0..columns.with(Vec::len)).collect::<Vec<_>>());
    view! {
        <unstyled::Table label>
            <List spacing=Space::None>
                <TableHeader heading={heading} columns={columns} indices={indices} />
                {children}
            </List>
        </unstyled::Table>
    }
}

#[component]
fn TableHeader(
    heading: Memo<TableColumn>,
    columns: Memo<Vec<TableColumn>>,
    indices: Memo<Vec<usize>>,
) -> NodeId {
    view! {
        <unstyled::TableRow>
            <Frame padding=CELL_PADDING>
                <List direction=Direction::Horizontal spacing=CELL_SPACING>
                    <ColumnHeading weight=LABEL_WEIGHT column={heading} />
                    <ForEach keys={indices}>|index: usize| {
                        let column = create_memo(move || {
                            columns.with(|columns| columns.get(index).cloned().unwrap_or_default())
                        });
                        view! {
                            <ColumnHeading weight=CELL_WEIGHT column={column} />
                        }
                    }</ForEach>
                </List>
            </Frame>
        </unstyled::TableRow>
    }
}

#[component]
fn ColumnHeading(weight: f32, column: Memo<TableColumn>) -> ListItem {
    let title = create_memo(move || column.with(|column| column.title.clone()));
    let caption = create_memo(move || column.with(|column| column.caption.clone()));
    let captioned = create_memo(move || !caption.with(String::is_empty));
    let label = create_memo(move || spoken(&title.get(), &caption.get()));
    view! {
        <ListItem grow=weight>
            <unstyled::TableColumnHeader label={label}>
                <List spacing=Space::None>
                    <Text style=COLUMN_TITLE>{title}</Text>
                    <Show condition={captioned}>
                        <Text style=TextStyle::CAPTION>{caption}</Text>
                    </Show>
                </List>
            </unstyled::TableColumnHeader>
        </ListItem>
    }
}

#[component]
pub fn TableRow(label: Prop<String>, children: Children<ListItem>) -> NodeId {
    let label = create_memo(move || label.get());
    view! {
        <unstyled::TableRow>
            <List spacing=Space::None>
                <Frame padding={Padding::horizontal(CELL_PADDING_HORIZONTAL)}>
                    <Separator />
                </Frame>
                <Frame padding=CELL_PADDING>
                    <List direction=Direction::Horizontal spacing=CELL_SPACING>
                        <RowHeading label={label} />
                        {children}
                    </List>
                </Frame>
            </List>
        </unstyled::TableRow>
    }
}

#[component]
fn RowHeading(label: Memo<String>) -> ListItem {
    view! {
        <ListItem grow=LABEL_WEIGHT>
            <unstyled::TableRowHeader label={label}>
                <Text>{label}</Text>
            </unstyled::TableRowHeader>
        </ListItem>
    }
}

#[component]
pub fn TableCell(
    #[prop(default = String::new())] text: Prop<String>,
    #[prop(default = String::new())] caption: Prop<String>,
    #[prop(default = TextColor::Default)] color: Prop<TextColor>,
) -> ListItem {
    let text = create_memo(move || text.get());
    let caption = create_memo(move || caption.get());
    let captioned = create_memo(move || !caption.get().is_empty());
    let label = create_memo(move || spoken(&text.get(), &caption.get()));
    view! {
        <ListItem grow=CELL_WEIGHT>
            <unstyled::TableCell label={label}>
                <List spacing=CAPTION_SPACING>
                    <Text color={color}>{text}</Text>
                    <Show condition={captioned}>
                        <Text style=TextStyle::CAPTION>{caption}</Text>
                    </Show>
                </List>
            </unstyled::TableCell>
        </ListItem>
    }
}

fn spoken(text: &str, caption: &str) -> String {
    match caption.is_empty() {
        true => text.to_owned(),
        false => format!("{text}, {caption}"),
    }
}
