use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::BlockList;
use block_editor_beui::BlockParent;
use block_editor_beui::BlockQuery;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::be_block::Edit;
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::database::{
    Database, DatabaseContent, DatabaseRow, DatabaseValue, RowRef,
};
use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseSchemaContent};
use block_editor_beui::be_block::database_view::{
    DatabaseViewContent, DatabaseViewKind, DatabaseViewSort, ViewFilter,
};
use block_editor_beui::be_block::text::{TextBlock, TextContent};
use block_editor_beui::beui::reactive::{
    Memo, WriteSignal, clone, create_effect, create_memo, create_signal,
};
use block_editor_beui::block_ui::BlockLabel;
use block_editor_beui::block_ui::database::DatabaseBlockPickRequest;
use block_editor_beui::be_block::database::Inverse;
use block_editor_beui::database::{relation_targets, watch_inverses, watch_row_labels};
use block_editor_beui::{BlockFilter, ContentProjection, Editor, RelatedContent};
use uuid::Uuid;

use crate::database_view::sort::BlockLabels;

pub type Data = Rc<ViewData>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub row: usize,
    pub field: Option<Uuid>,
}

pub struct ViewData {
    editor: Editor,
    view: Rc<ContentProjection<DatabaseViewContent>>,
    database: Rc<RelatedContent<DatabaseContent>>,
    set_selected: WriteSignal<Option<Selection>>,
    set_error: WriteSignal<Option<String>>,
    pub schema_id: Memo<Option<Uuid>>,
    pub database_id: Memo<Option<Uuid>>,
    pub inverses: Memo<Vec<Inverse>>,
    pub rows: Memo<Vec<DatabaseRow>>,
    pub row_ids: Memo<Vec<ObjectId>>,
    pub prefix: Memo<String>,
    pub title_field: Memo<Option<Uuid>>,
    pub fields: Memo<Vec<DatabaseField>>,
    pub labels: Memo<BlockLabels>,
    pub kind: Memo<DatabaseViewKind>,
    pub sort: Memo<Option<DatabaseViewSort>>,
    pub filter_items: Memo<Vec<(ObjectId, ViewFilter)>>,
    pub filters: Memo<Vec<ViewFilter>>,
    pub kanban_field: Memo<Option<Uuid>>,
    pub scatter_x: Memo<Option<Uuid>>,
    pub scatter_y: Memo<Option<Uuid>>,
    pub selected: Memo<Option<Selection>>,
    pub error: Memo<Option<String>>,
    pub read_only: Memo<bool>,
}

impl ViewData {
    pub fn new(editor: &Editor) -> Data {
        let view = editor.block_content::<DatabaseViewContent>();
        let database_ref = view.project(|view| view.root().database);
        let database_id = create_memo(move || database_ref.get());
        let database = editor.related_content::<DatabaseContent>(database_id.clone());
        let schema_ref = database.project(|database| database.root().schema);
        let schema_id = create_memo(move || schema_ref.get());
        let schema = editor.related_content::<DatabaseSchemaContent>(schema_id.clone());

        let rows = database.project(|database| {
            database
                .root()
                .rows
                .iter()
                .map(|row| row.value.clone())
                .collect::<Vec<DatabaseRow>>()
        });
        let row_ids = database.project(|database| {
            database
                .root()
                .rows
                .iter()
                .map(|row| row.id)
                .collect::<Vec<ObjectId>>()
        });
        let row_ids = create_memo(clone!(row_ids -> move || row_ids.get()));
        let inverses = watch_inverses(editor, database_id.clone());
        let rows = create_memo(clone!(rows row_ids inverses -> move || {
            let ids = row_ids.get();
            let inverses = inverses.get();
            rows.get()
                .into_iter()
                .zip(ids)
                .map(|(row, id)| {
                    inverses
                        .iter()
                        .fold(row, |row, inverse| inverse.shown_on(&row, id.as_uuid()))
                })
                .collect::<Vec<DatabaseRow>>()
        }));
        let prefix = database.project(|database| database.root().prefix.clone());
        let prefix = create_memo(clone!(prefix -> move || prefix.get()));
        let title_field = schema.project(|schema| schema.root().title_field());
        let title_field = create_memo(clone!(title_field -> move || title_field.get()));
        let fields = schema.project(|schema| schema.root().fields());
        let fields = create_memo(clone!(fields inverses -> move || {
            let mut fields = fields.get();
            fields.extend(inverses.with(|inverses| {
                inverses.iter().map(Inverse::field).collect::<Vec<_>>()
            }));
            fields
        }));
        let kind = view.project(|view| view.root().kind);
        let kind = create_memo(clone!(kind -> move || kind.get()));
        let sort = view.project(|view| view.root().sort);
        let sort = create_memo(clone!(sort -> move || sort.get()));
        let kanban_field = view.project(|view| view.root().kanban_field);
        let kanban_field = create_memo(clone!(kanban_field -> move || kanban_field.get()));
        let scatter_x = view.project(|view| view.root().scatter_x);
        let scatter_x = create_memo(clone!(scatter_x -> move || scatter_x.get()));
        let scatter_y = view.project(|view| view.root().scatter_y);
        let scatter_y = create_memo(clone!(scatter_y -> move || scatter_y.get()));
        let filter_items = view.project(|view| {
            view.root()
                .filters
                .iter()
                .map(|filter| (filter.id, filter.value.clone()))
                .collect::<Vec<(ObjectId, ViewFilter)>>()
        });
        let filter_items = create_memo(clone!(filter_items -> move || filter_items.get()));
        let filters = create_memo(clone!(filter_items -> move || {
            filter_items.with(|items| items.iter().map(|(_, filter)| filter.clone()).collect())
        }));
        let targets =
            create_memo(clone!(fields -> move || fields.with(|fields| relation_targets(fields))));
        let row_labels = watch_row_labels(editor, targets);
        let labels = watch_labels(editor, database_id.clone(), rows.clone(), row_labels);

        let (selected, set_selected) = create_signal(None::<Selection>);
        let (error, set_error) = create_signal(None::<String>);

        Rc::new(Self {
            editor: editor.clone(),
            view,
            database,
            set_selected,
            set_error,
            schema_id,
            database_id,
            inverses,
            rows,
            row_ids,
            prefix,
            title_field,
            fields,
            labels,
            kind,
            sort,
            filter_items,
            filters,
            kanban_field,
            scatter_x,
            scatter_y,
            selected: create_memo(move || selected.get()),
            error: create_memo(move || error.get()),
            read_only: editor.read_only(),
        })
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn select(&self, row: usize, field: Option<Uuid>) {
        self.set_selected.set(Some(Selection { row, field }));
    }

    pub fn step_selection(&self, rows: isize) {
        let Some(selection) = self.selected.get_untracked() else {
            return;
        };
        let display = self.rows.with_untracked(|shown| {
            self.fields.with_untracked(|fields| {
                self.labels.with_untracked(|labels| {
                    self.filters.with_untracked(|filters| {
                        crate::database_view::sort::display_rows(
                            shown,
                            Some(selection.row),
                            self.sort.get_untracked(),
                            filters,
                            fields,
                            labels,
                        )
                    })
                })
            })
        });
        if display.is_empty() {
            return;
        }
        let position = display
            .iter()
            .position(|row| row.index == selection.row)
            .unwrap_or(0);
        let next = position.saturating_add_signed(rows).min(display.len() - 1);
        self.select(display[next].index, selection.field);
    }

    pub fn deselect(&self) {
        self.set_selected.set(None);
    }

    pub fn dismiss_error(&self) {
        self.set_error.set(None);
    }

    pub fn operate_view(&self, edit: Edit) {
        self.view.operate(edit);
    }

    pub fn shows(&self, row: &DatabaseRow) -> bool {
        self.filters
            .with(|filters| filters.iter().all(|filter| filter.matches(row)))
    }

    pub fn open_page(&self, index: usize) {
        let Some(row) = self.row_ids.with_untracked(|ids| ids.get(index).copied()) else {
            return;
        };
        let page = match self.page_of(row) {
            Some(page) => Some(page),
            None if !self.read_only.get_untracked() => {
                let title = self.title_field.get_untracked().and_then(|field| {
                    self.rows
                        .with_untracked(|rows| match rows.get(index)?.value(field)? {
                            DatabaseValue::String(text) => Some(text.clone()),
                            _ => None,
                        })
                });
                self.create_page(row, title.unwrap_or_default())
            }
            None => None,
        };
        if let Some(page) = page {
            self.editor
                .host()
                .open_block(page, TextContent::CONTENT_TYPE);
        }
    }

    pub fn index_of(&self, row: ObjectId) -> Option<usize> {
        self.row_ids
            .with_untracked(|ids| ids.iter().position(|id| *id == row))
    }

    pub fn add_row(&self, values: Vec<(Uuid, DatabaseValue)>) -> ObjectId {
        let (row, edit) = Database::add_row(values);
        self.database.operate(edit);
        row
    }

    pub fn set_value(&self, row: ObjectId, field: Uuid, value: Option<DatabaseValue>) {
        let inverse = self
            .inverses
            .with_untracked(|inverses| inverses.iter().find(|inverse| inverse.id == field).cloned());
        if let Some(inverse) = inverse {
            self.set_inverse(&inverse, row, value);
            return;
        }
        let edit = self.database.read(|database| {
            let database = database.root();
            match value {
                Some(DatabaseValue::Links(targets)) => database.set_links(row, field, &targets),
                None => Database::set_value(row, field, None)
                    .0
                    .into_iter()
                    .chain(database.set_links(row, field, &[]).0)
                    .collect(),
                Some(value) => Database::set_value(row, field, Some(&value)),
            }
        });
        if let Some(edit) = edit {
            self.database.operate(edit);
        }
    }

    pub fn set_prefix(&self, prefix: String) {
        self.database.operate(Database::set_prefix(prefix));
    }

    pub fn page_of(&self, row: ObjectId) -> Option<Uuid> {
        self.database
            .read(|database| database.root().rows.get(row).and_then(|row| row.page()))
            .flatten()
    }

    pub fn create_page(&self, row: ObjectId, name: String) -> Option<Uuid> {
        let database = self.database_id.get_untracked()?;
        let page = self.editor.blocks().create_named(
            &TextBlock::new("").content(),
            BlockParent::Block(database),
            name,
        );
        self.database.operate(Database::set_page(row, Some(page)));
        Some(page)
    }

    pub fn remove_row(&self, row: ObjectId) {
        if let Some(page) = self.page_of(row) {
            self.editor.blocks().set_parent(page, BlockParent::Detached);
        }
        if self
            .selected
            .get_untracked()
            .is_some_and(|selection| Some(selection.row) == self.index_of(row))
        {
            self.deselect();
        }
        self.database.operate(Database::remove_row(row));
    }

    fn set_inverse(&self, inverse: &Inverse, row: ObjectId, value: Option<DatabaseValue>) {
        let Some(database) = self.database_id.get_untracked() else {
            return;
        };
        let wanted = match value {
            Some(DatabaseValue::Links(links)) => links,
            Some(DatabaseValue::Relation(link)) => vec![link],
            _ => Vec::new(),
        };
        let target = RowRef {
            database,
            row: row.as_uuid(),
        };
        let source = self.editor.content_of::<DatabaseContent>(inverse.source);
        let edit = source.read(|source| inverse.edits(&source.root(), target, &wanted));
        if let Some(edit) = edit.filter(|edit| !edit.0.is_empty()) {
            source.operate(edit);
        }
    }

    pub fn set_cell(&self, row_index: usize, field_id: Uuid, value: Option<DatabaseValue>) {
        let existing = self.row_ids.with_untracked(|ids| ids.get(row_index).copied());
        match (existing, &value) {
            (Some(row), Some(DatabaseValue::Links(_))) => self.set_value(row, field_id, value),
            (None, Some(DatabaseValue::Links(_))) => {}
            (existing, _) => {
                if let Some(row) = existing.filter(|_| value.is_none()) {
                    let unlinked = self
                        .database
                        .read(|database| database.root().set_links(row, field_id, &[]));
                    if let Some(unlinked) = unlinked.filter(|edit| !edit.0.is_empty()) {
                        self.database.operate(unlinked);
                    }
                }
                set_cell(&self.database, row_index, field_id, value);
            }
        }
    }

    pub fn pick_value(&self, row: usize, request: DatabaseBlockPickRequest) {
        let Some(name) = self.fields.with_untracked(|fields| {
            fields
                .iter()
                .find(|field| field.id == request.field_id)
                .map(|field| field.name.clone())
        }) else {
            return;
        };
        let database = Rc::clone(&self.database);
        let set_error = self.set_error.clone();
        self.editor.pick_block(
            value_block_filter(&name, request),
            move |picked| match picked {
                Ok(picked) => set_cell(
                    &database,
                    row,
                    request.field_id,
                    Some(DatabaseValue::Block(picked.id)),
                ),
                Err(error) => set_error.set(Some(error)),
            },
        );
    }
}

fn set_cell(
    database: &RelatedContent<DatabaseContent>,
    row_index: usize,
    field_id: Uuid,
    value: Option<DatabaseValue>,
) {
    if let Some(edit) =
        database.read(|database| database.root().set_cell(row_index, field_id, value))
    {
        database.operate(edit);
    }
}

pub fn value_block_filter(field_name: &str, request: DatabaseBlockPickRequest) -> BlockFilter {
    BlockFilter {
        name: field_name.to_owned(),
        block_types: request
            .block_type
            .into_iter()
            .map(Uuid::into_bytes)
            .collect(),
        excluded: Vec::new(),
        templates: false,
        place: None,
    }
}

fn watch_labels(
    editor: &Editor,
    database_id: Memo<Option<Uuid>>,
    rows: Memo<Vec<DatabaseRow>>,
    row_labels: Memo<std::collections::BTreeMap<RowRef, String>>,
) -> Memo<BlockLabels> {
    let (labels, set_labels) = create_signal(HashMap::<Uuid, BlockLabel>::new());
    let watched: RefCell<Option<(Uuid, BlockList)>> = RefCell::new(None);
    let client = editor.blocks();
    let catalog = editor.clone();
    create_effect(move || {
        let Some(database_id) = database_id.get() else {
            return;
        };
        let mut watched = watched.borrow_mut();
        if watched.as_ref().is_none_or(|(id, _)| *id != database_id) {
            *watched = Some((
                database_id,
                client.watch(BlockQuery::References(database_id)),
            ));
        }
        let Some((_, references)) = watched.as_ref() else {
            return;
        };
        let types = catalog.block_types();
        let known: HashMap<Uuid, BlockLabel> = references
            .read()
            .into_iter()
            .map(|reference| (reference.id, reference.label(types.as_ref())))
            .collect();
        set_labels.set(rows.with(|rows| {
            row_references(rows)
                .into_iter()
                .filter_map(|reference| Some((reference, known.get(&reference)?.clone())))
                .collect()
        }));
    });
    create_memo(move || BlockLabels {
        blocks: labels.get(),
        rows: row_labels.get(),
    })
}

fn row_references(rows: &[DatabaseRow]) -> Vec<Uuid> {
    rows.iter()
        .flat_map(|row| row.values().values())
        .filter_map(|value| match value {
            DatabaseValue::Block(reference) => Some(*reference),
            _ => None,
        })
        .collect()
}
