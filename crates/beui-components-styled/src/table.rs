use crate::border::Separator;
use crate::frame::Frame;
use crate::text::{Text, TextColor, TextStyle};
use accesskit::{Node, Role};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;

const LABEL_WEIGHT: f32 = 4.0;
const CELL_WEIGHT: f32 = 3.0;
const CELL_PADDING_HORIZONTAL: f32 = 16.0;
const CELL_PADDING_VERTICAL: f32 = 10.0;
const CELL_SPACING: f32 = 12.0;
const CAPTION_SPACING: f32 = 2.0;
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
    children: Children<ListChild>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let heading = create_memo(move || TableColumn::new(heading.get()));
    let columns = create_memo(move || columns.get());
    let indices = create_memo(move || (0..columns.with(Vec::len)).collect::<Vec<_>>());
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Table);
        node.set_label(label.get());
        node
    }));
    view! {
        <List spacing=0.0>
            <TableHeader heading={heading} columns={columns} indices={indices} />
            {children}
        </List>
    }
}

#[component]
fn TableHeader(
    heading: Memo<TableColumn>,
    columns: Memo<Vec<TableColumn>>,
    indices: Memo<Vec<usize>>,
) -> NodeId {
    component_accessibility(Node::new(Role::Row));
    view! {
        <Frame padding={Sides::symmetric(CELL_PADDING_HORIZONTAL, CELL_PADDING_VERTICAL)}>
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
    }
}

#[component]
fn ColumnHeading(weight: f32, column: Memo<TableColumn>) -> ListChild {
    let title = create_memo(move || column.with(|column| column.title.clone()));
    let caption = create_memo(move || column.with(|column| column.caption.clone()));
    let captioned = create_memo(move || !caption.with(String::is_empty));
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::ColumnHeader);
        node.set_label(spoken(&title.get(), &caption.get()));
        node
    }));
    view! {
        <List @sizing=ItemSize::Percent(weight) spacing=0.0>
            <Text style=COLUMN_TITLE>{title}</Text>
            <Show condition={captioned}>
                <Text style=TextStyle::CAPTION>{caption}</Text>
            </Show>
        </List>
    }
}

#[component]
pub fn TableRow(label: Prop<String>, children: Children<ListChild>) -> NodeId {
    let label = create_memo(move || label.get());
    component_accessibility(Node::new(Role::Row));
    view! {
        <List spacing=0.0>
            <Frame padding={Sides::horizontal(CELL_PADDING_HORIZONTAL)}>
                <Separator />
            </Frame>
            <Frame padding={Sides::symmetric(CELL_PADDING_HORIZONTAL, CELL_PADDING_VERTICAL)}>
                <List direction=Direction::Horizontal spacing=CELL_SPACING>
                    <RowHeading label={label} />
                    {children}
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn RowHeading(label: Memo<String>) -> ListChild {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::RowHeader);
        node.set_label(label.get());
        node
    }));
    view! {
        <List @sizing=ItemSize::Percent(LABEL_WEIGHT) spacing=0.0>
            <Text>{label}</Text>
        </List>
    }
}

#[component]
pub fn TableCell(
    #[prop(default = String::new())] text: Prop<String>,
    #[prop(default = String::new())] caption: Prop<String>,
    #[prop(default = TextColor::Default)] color: Prop<TextColor>,
) -> ListChild {
    let text = create_memo(move || text.get());
    let caption = create_memo(move || caption.get());
    let captioned = create_memo(move || !caption.get().is_empty());
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Cell);
        node.set_label(spoken(&text.get(), &caption.get()));
        node
    }));
    view! {
        <List @sizing=ItemSize::Percent(CELL_WEIGHT) spacing=CAPTION_SPACING>
            <Text color={color}>{text}</Text>
            <Show condition={captioned}>
                <Text style=TextStyle::CAPTION>{caption}</Text>
            </Show>
        </List>
    }
}

fn spoken(text: &str, caption: &str) -> String {
    match caption.is_empty() {
        true => text.to_owned(),
        false => format!("{text}, {caption}"),
    }
}
