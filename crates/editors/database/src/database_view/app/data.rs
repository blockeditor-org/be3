use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_beui::BlockInfo;
use block_editor_beui::BlockParent;
use block_editor_beui::BlockQuery;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::be_block::Edit;
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::block_ui::BlockLabel;
use block_editor_beui::database::DatabaseBlockPickRequest;
use block_editor_beui::database::{relation_targets, watch_inverses, watch_row_labels};
use block_editor_beui::{BlockFilter, ContentProjection, Editor, RelatedContent};
use database_types::database::Inverse;
use database_types::database::{
    Database, DatabaseContent, DatabaseRow, DatabaseValue, RowRef, short_id,
};
use database_types::database_schema::{DatabaseField, DatabaseSchemaContent};
use database_types::database_view::{
    DatabaseView, DatabaseViewContent, DatabaseViewKind, DatabaseViewSort, ViewFilter,
};
use uuid::Uuid;

use crate::database_view::groups::{Group, GroupKey, group_rows, matches_search};
use crate::database_view::sort::BlockLabels;
use text_block_types::{TextBlock, TextContent};

pub type Data = ViewData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub row: usize,
    pub field: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub struct ViewData {
    editor: Editor,
    view: ContentProjection<DatabaseViewContent>,
    database: RelatedContent<DatabaseContent>,
    pub schema_id: Memo<Option<Uuid>>,
    pub database_id: Memo<Option<Uuid>>,
    pub inverses: Memo<Vec<Inverse>>,
    pub rows: Memo<Vec<DatabaseRow>>,
    pub row_ids: Memo<Vec<ObjectId>>,
    pub row_index: Memo<HashMap<ObjectId, usize>>,
    pub prefix: Memo<String>,
    pub title_field: Memo<Option<Uuid>>,
    pub fields: Memo<Vec<DatabaseField>>,
    pub labels: Memo<BlockLabels>,
    pub kind: Memo<DatabaseViewKind>,
    pub sort: Memo<Option<DatabaseViewSort>>,
    pub filter_items: Memo<Vec<(ObjectId, ViewFilter)>>,
    pub filters: Memo<Vec<ViewFilter>>,
    pub group_by: Memo<Option<Uuid>>,
    pub hidden_fields: Memo<Vec<Uuid>>,
    pub show_empty_groups: Memo<bool>,
    pub defaults: Memo<Vec<(Uuid, DatabaseValue)>>,
    pub search: Signal<String>,
    pub toggled: Signal<HashSet<GroupKey>>,
    pub views: Memo<Vec<(Uuid, String)>>,
    pub name: Memo<String>,
    pub groups: Memo<Vec<Group>>,
    pub scatter_x: Memo<Option<Uuid>>,
    pub scatter_y: Memo<Option<Uuid>>,
    pub selected: Signal<Option<Selection>>,
    pub error: Signal<Option<String>>,
    pub read_only: Memo<bool>,
}

impl ViewData {
    pub fn new(editor: Editor) -> Data {
        let view = editor.block_content::<DatabaseViewContent>();
        let database_ref = view.project(|view| view.root().database);
        let database_id = create_memo(move || database_ref.get());
        let database = editor.related_content::<DatabaseContent>(database_id);
        let schema_ref = database.field(ObjectId::ROOT, Database::SCHEMA);
        let schema_id = create_memo(move || schema_ref.get());
        let schema = editor.related_content::<DatabaseSchemaContent>(schema_id);

        let stored = database.objects(ObjectId::ROOT, Database::ROWS);
        let row_ids = create_memo(move || {
            stored.with(|rows| rows.iter().map(|(id, _)| *id).collect::<Vec<ObjectId>>())
        });
        let row_index = create_memo(move || {
            row_ids.with(|ids| {
                ids.iter()
                    .enumerate()
                    .map(|(index, id)| (*id, index))
                    .collect::<HashMap<ObjectId, usize>>()
            })
        });
        let found_inverses = watch_inverses(editor, database_id);
        let inverses = create_memo(move || found_inverses.get().unwrap_or_default());
        let rows = create_memo(move || {
            inverses.with(|inverses| {
                stored.with(|rows| {
                    rows.iter()
                        .map(|(id, row)| {
                            let mut row = row.as_ref().clone();
                            for inverse in inverses {
                                inverse.show_on(&mut row, id.as_uuid());
                            }
                            row
                        })
                        .collect::<Vec<DatabaseRow>>()
                })
            })
        });
        let prefix = database.field(ObjectId::ROOT, Database::PREFIX);
        let prefix = create_memo(move || prefix.get());
        let title_field = schema.project(|schema| schema.root().title_field());
        let title_field = create_memo(move || title_field.get());
        let fields = schema.project(|schema| schema.root().fields());
        let fields = create_memo(move || {
            let mut fields = fields.get();
            fields.extend(
                inverses.with(|inverses| inverses.iter().map(Inverse::field).collect::<Vec<_>>()),
            );
            fields
        });
        let kind = view.project(|view| view.root().kind);
        let kind = create_memo(move || kind.get());
        let sort = view.project(|view| view.root().sort);
        let sort = create_memo(move || sort.get());
        let group_by = view.project(|view| view.root().group_by);
        let group_by = create_memo(move || group_by.get());
        let hidden_fields = view.project(|view| view.root().hidden_fields.clone());
        let hidden_fields = create_memo(move || hidden_fields.get());
        let show_empty_groups = view.project(|view| view.root().show_empty_groups);
        let show_empty_groups = create_memo(move || show_empty_groups.get());
        let defaults = schema.project(|schema| schema.root().defaults());
        let defaults = create_memo(move || defaults.get());
        let scatter_x = view.project(|view| view.root().scatter_x);
        let scatter_x = create_memo(move || scatter_x.get());
        let scatter_y = view.project(|view| view.root().scatter_y);
        let scatter_y = create_memo(move || scatter_y.get());
        let filter_items = view.project(|view| {
            view.root()
                .filters
                .iter()
                .map(|filter| (filter.id, filter.value.clone()))
                .collect::<Vec<(ObjectId, ViewFilter)>>()
        });
        let filter_items = create_memo(move || filter_items.get());
        let filters = create_memo(move || {
            filter_items.with(|items| {
                items
                    .iter()
                    .map(|(_, filter)| filter.clone())
                    .collect::<Vec<ViewFilter>>()
            })
        });
        let targets = create_memo(move || fields.with(|fields| relation_targets(fields)));
        let row_labels = watch_row_labels(editor, targets);
        let labels = watch_labels(editor, database_id, rows, row_labels);

        let selected = create_signal(None::<Selection>);
        let error = create_signal(None::<String>);
        let search = create_signal(String::new());
        let toggled = create_signal(HashSet::<GroupKey>::new());
        let views = watch_views(editor, database_id);
        let name = watch_name(editor, database_id);
        let groups = create_memo(move || {
            let search = search.get();
            let prefix = prefix.get();
            let ids = row_ids.get();
            let title = title_field.get();
            let group_by = group_by.get();
            let show_empty = show_empty_groups.get() || kind.get() == DatabaseViewKind::Kanban;
            let sort = sort.get();
            rows.with(|rows| {
                fields.with(|fields| {
                    labels.with(|labels| {
                        filters.with(|filters| {
                            let shown: Vec<usize> = crate::database_view::sort::display_rows(
                                rows, None, sort, filters, fields, labels,
                            )
                            .into_iter()
                            .map(|row| row.index)
                            .filter(|index| *index < rows.len())
                            .filter(|index| {
                                let code = ids
                                    .get(*index)
                                    .map(|id| short_id(&prefix, id.as_uuid()))
                                    .unwrap_or_default();
                                matches_search(&rows[*index], title, &code, &search)
                            })
                            .collect();
                            let field = group_by.and_then(|id| fields.iter().find(|f| f.id == id));
                            group_rows(rows, &shown, field, labels, show_empty)
                        })
                    })
                })
            })
        });

        Self {
            editor,
            view,
            database,
            schema_id,
            database_id,
            inverses,
            rows,
            row_ids,
            row_index,
            prefix,
            title_field,
            fields,
            labels,
            kind,
            sort,
            filter_items,
            filters,
            group_by,
            hidden_fields,
            show_empty_groups,
            defaults,
            search,
            groups,
            toggled,
            views,
            name,
            scatter_x,
            scatter_y,
            selected,
            error,
            read_only: editor.read_only(),
        }
    }

    pub fn editor(&self) -> Editor {
        self.editor
    }

    pub fn select(&self, row: usize, field: Option<Uuid>) {
        self.selected.set(Some(Selection { row, field }));
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
        self.selected.set(None);
    }

    pub fn dismiss_error(&self) {
        self.error.set(None);
    }

    pub fn operate_view(&self, edit: Edit) {
        self.view.operate(edit);
    }

    pub fn shows(&self, row: &DatabaseRow) -> bool {
        self.fields.with(|fields| {
            self.filters
                .with(|filters| filters.iter().all(|filter| filter.matches(row, fields)))
        })
    }

    pub fn set_search(&self, query: String) {
        self.search.set(query);
    }

    pub fn toggle_group(&self, key: GroupKey) {
        self.toggled.update(|toggled| {
            if !toggled.remove(&key) {
                toggled.insert(key);
            }
        });
    }

    pub fn select_row(&self, row: ObjectId) {
        if let Some(index) = self.index_of(row) {
            self.select(index, self.title_field.get_untracked());
        }
    }

    pub fn selected_row(&self) -> Option<ObjectId> {
        let selection = self.selected.get()?;
        self.row_ids.with(|ids| ids.get(selection.row).copied())
    }

    pub fn new_row(&self, mut values: Vec<(Uuid, DatabaseValue)>) -> ObjectId {
        for (field, value) in self.defaults.get_untracked() {
            if values.iter().all(|(set, _)| *set != field) {
                values.push((field, value));
            }
        }
        let before = self.row_ids.with_untracked(Vec::len);
        let row = self.add_row(values);
        let index = self.index_of(row).unwrap_or(before);
        self.select(index, self.title_field.get_untracked());
        row
    }

    pub fn new_view(&self) {
        let Some(database) = self.database_id.get_untracked() else {
            return;
        };
        let blocks = self.editor.blocks();
        let parent = blocks
            .info(self.editor.block_id())
            .and_then(|info| info.parent)
            .unwrap_or(BlockParent::Detached);
        let mut view = DatabaseView::of(database);
        view.kind = DatabaseViewKind::List;
        view.group_by = self.group_by.get_untracked();
        let count = self.views.with_untracked(Vec::len);
        let created = blocks.create_named(
            &DatabaseViewContent::new(&view),
            BlockParent::Detached,
            format!("View {}", count + 1),
        );
        match parent {
            BlockParent::Block(container) => self.editor.host().place_block(
                created,
                DatabaseViewContent::CONTENT_TYPE,
                container,
                false,
            ),
            other => blocks.set_parent(created, other),
        }
        self.editor
            .host()
            .open_block(created, DatabaseViewContent::CONTENT_TYPE);
    }

    pub fn open_view(&self, view: Uuid) {
        if view != self.editor.block_id() {
            self.editor
                .host()
                .open_block(view, DatabaseViewContent::CONTENT_TYPE);
        }
    }

    pub fn open_schema(&self) {
        if let Some(schema) = self.schema_id.get_untracked() {
            self.editor
                .host()
                .open_block(schema, DatabaseSchemaContent::CONTENT_TYPE);
        }
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
        self.row_index
            .with_untracked(|indices| indices.get(&row).copied())
    }

    pub fn add_row(&self, values: Vec<(Uuid, DatabaseValue)>) -> ObjectId {
        let (row, edit) = Database::add_row(values);
        self.database.operate(edit);
        row
    }

    pub fn set_value(&self, row: ObjectId, field: Uuid, value: Option<DatabaseValue>) {
        let inverse = self.inverses.with_untracked(|inverses| {
            inverses.iter().find(|inverse| inverse.id == field).cloned()
        });
        if let Some(inverse) = inverse {
            self.set_inverse(&inverse, row, value);
            return;
        }
        let edit = match value {
            Some(DatabaseValue::Links(targets)) => self
                .database
                .read(|database| database.root().set_links(row, field, &targets)),
            None => self.database.read(|database| {
                Database::set_value(row, field, None)
                    .0
                    .into_iter()
                    .chain(database.root().set_links(row, field, &[]).0)
                    .collect()
            }),
            Some(value) => Some(Database::set_value(row, field, Some(&value))),
        };
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
        let Some(database) = self.database_id.get_untracked() else {
            return;
        };
        if let Some(edit) = self
            .database
            .read(|content| content.root().remove_row_and_links(database, row))
        {
            self.database.operate(edit);
        }
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
        if let Some(edit) = edit.ok().filter(|edit| !edit.0.is_empty()) {
            source.operate(edit);
        }
    }

    pub fn set_cell(&self, row_index: usize, field_id: Uuid, value: Option<DatabaseValue>) {
        let existing = self
            .row_ids
            .with_untracked(|ids| ids.get(row_index).copied());
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
        let database = self.database;
        let error = self.error;
        self.editor.pick_block(
            value_block_filter(&name, request),
            move |picked| match picked {
                Ok(picked) => set_cell(
                    &database,
                    row,
                    request.field_id,
                    Some(DatabaseValue::Block(picked.id())),
                ),
                Err(failure) => error.set(Some(failure.to_string())),
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
        anchor: request.anchor,
    }
}

fn watch_database_blocks<T: Clone + PartialEq + Default + 'static>(
    editor: Editor,
    database_id: Memo<Option<Uuid>>,
    query: fn(Uuid) -> BlockQuery,
    derive: impl Fn(Vec<BlockInfo>) -> T + 'static,
) -> Memo<T> {
    let found = create_signal(T::default());
    let derive = Rc::new(derive);
    let client = editor.blocks();
    create_binding(move || {
        let Some(database_id) = database_id.get() else {
            return;
        };
        let list = client.watch(query(database_id));
        let derive = Rc::clone(&derive);
        create_binding(move || {
            if let Ok(blocks) = list.read() {
                found.set(derive(blocks));
            }
        });
    });
    create_memo(move || found.get())
}

fn watch_labels(
    editor: Editor,
    database_id: Memo<Option<Uuid>>,
    rows: Memo<Vec<DatabaseRow>>,
    row_labels: block_editor_beui::database::RowLabels,
) -> Memo<BlockLabels> {
    let labels = watch_database_blocks(
        editor,
        database_id,
        BlockQuery::References,
        move |references| {
            let types = editor.block_types();
            let known: HashMap<Uuid, BlockLabel> = references
                .into_iter()
                .map(|reference| (reference.id, reference.label(types.as_ref())))
                .collect();
            rows.with(|rows| {
                row_references(rows)
                    .into_iter()
                    .filter_map(|reference| Some((reference, known.get(&reference)?.clone())))
                    .collect::<HashMap<Uuid, BlockLabel>>()
            })
        },
    );
    create_memo(move || BlockLabels {
        blocks: labels.get(),
        rows: row_labels.labels.get(),
        unreachable: row_labels.unreachable.get(),
    })
}

fn watch_views(editor: Editor, database_id: Memo<Option<Uuid>>) -> Memo<Vec<(Uuid, String)>> {
    let own = editor.block_id();
    watch_database_blocks(editor, database_id, BlockQuery::Backrefs, move |backrefs| {
        let mut found: Vec<(Uuid, String)> = backrefs
            .into_iter()
            .filter(|info| info.block_type == DatabaseViewContent::CONTENT_TYPE)
            .map(|info| (info.id, info.name().unwrap_or_default()))
            .collect();
        if found.iter().all(|(id, _)| *id != own) {
            found.insert(0, (own, String::new()));
        }
        found.sort_by_key(|(id, _)| *id != own);
        let count = found.len();
        for (index, (_, name)) in found.iter_mut().enumerate() {
            if name.trim().is_empty() {
                *name = match count {
                    1 => "All".to_owned(),
                    _ => format!("View {}", index + 1),
                };
            }
        }
        found
    })
}

fn watch_name(editor: Editor, database_id: Memo<Option<Uuid>>) -> Memo<String> {
    watch_database_blocks(editor, database_id, BlockQuery::Block, |block| {
        block
            .into_iter()
            .next()
            .and_then(|info| info.name())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Database".to_owned())
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
