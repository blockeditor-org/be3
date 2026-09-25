use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_plugin::BlockList;
use block_editor_plugin::BlockQuery;
use block_editor_plugin::be_block::Edit;
use block_editor_plugin::be_block::database::{DatabaseContent, DatabaseRow, DatabaseValue};
use block_editor_plugin::be_block::database_schema::{DatabaseField, DatabaseSchemaContent};
use block_editor_plugin::be_block::database_view::{
    DatabaseViewContent, DatabaseViewKind, DatabaseViewSort,
};
use block_editor_plugin::beui::reactive::{Memo, WriteSignal, clone, create_memo, create_signal};
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::block_ui::database::DatabaseBlockPickRequest;
use block_editor_plugin::{BlockFilter, ContentProjection, Editor, RelatedContent};
use uuid::Uuid;

use crate::sort::BlockLabels;

pub type Data = Rc<ViewData>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub row: usize,
    pub field: Option<Uuid>,
}

type Pending = Rc<RefCell<Vec<(Uuid, (usize, Uuid))>>>;

pub struct ViewData {
    editor: Editor,
    view: Rc<ContentProjection<DatabaseViewContent>>,
    database: Rc<RelatedContent<DatabaseContent>>,
    pending: Pending,
    set_selected: WriteSignal<Option<Selection>>,
    set_error: WriteSignal<Option<String>>,
    pub schema_id: Memo<Option<Uuid>>,
    pub rows: Memo<Vec<DatabaseRow>>,
    pub fields: Memo<Vec<DatabaseField>>,
    pub labels: Memo<BlockLabels>,
    pub kind: Memo<DatabaseViewKind>,
    pub sort: Memo<Option<DatabaseViewSort>>,
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
        let rows = create_memo(clone!(rows -> move || rows.get()));
        let fields = schema.project(|schema| schema.root().fields());
        let fields = create_memo(clone!(fields -> move || fields.get()));
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
        let labels = watch_labels(editor, database_id, rows.clone());

        let (selected, set_selected) = create_signal(None::<Selection>);
        let (error, set_error) = create_signal(None::<String>);
        let data = Rc::new(Self {
            editor: editor.clone(),
            view,
            database,
            pending: Pending::default(),
            set_selected,
            set_error,
            schema_id,
            rows,
            fields,
            labels,
            kind,
            sort,
            kanban_field,
            scatter_x,
            scatter_y,
            selected: create_memo(move || selected.get()),
            error: create_memo(move || error.get()),
            read_only: editor.read_only(),
        });
        let polled = Rc::clone(&data);
        editor.each_frame(move || polled.poll_pending());
        data
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
                    crate::sort::display_rows(
                        shown,
                        Some(selection.row),
                        self.sort.get_untracked(),
                        fields,
                        labels,
                    )
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

    pub fn set_cell(&self, row_index: usize, field_id: Uuid, value: Option<DatabaseValue>) {
        if let Some(edit) = self
            .database
            .read(|database| database.root().set_cell(row_index, field_id, value))
        {
            self.database.operate(edit);
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
        let pending = Rc::clone(&self.pending);
        let set_error = self.set_error.clone();
        self.editor.pick_block(
            value_block_filter(&name, request),
            move |picked| match picked {
                Ok(picked) => pending
                    .borrow_mut()
                    .push((picked.id, (row, request.field_id))),
                Err(error) => set_error.set(Some(error)),
            },
        );
    }

    fn poll_pending(&self) {
        let finished = std::mem::take(&mut *self.pending.borrow_mut());
        for (reference, (row_index, field_id)) in finished {
            self.set_cell(row_index, field_id, Some(DatabaseValue::Block(reference)));
        }
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
    }
}

fn watch_labels(
    editor: &Editor,
    database_id: Memo<Option<Uuid>>,
    rows: Memo<Vec<DatabaseRow>>,
) -> Memo<BlockLabels> {
    let (labels, set_labels) = create_signal(BlockLabels::new());
    let watched: RefCell<Option<(Uuid, BlockList)>> = RefCell::new(None);
    let client = editor.blocks();
    let host = editor.host().clone();
    editor.each_frame(move || {
        let Some(database_id) = database_id.get_untracked() else {
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
        let types = host.block_types();
        let known: HashMap<Uuid, BlockLabel> = references
            .read()
            .into_iter()
            .map(|reference| (reference.id, reference.label(types.as_ref())))
            .collect();
        set_labels.set(rows.with_untracked(|rows| {
            row_references(rows)
                .into_iter()
                .filter_map(|reference| Some((reference, known.get(&reference)?.clone())))
                .collect()
        }));
    });
    create_memo(move || labels.get())
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
