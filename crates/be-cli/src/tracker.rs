use std::collections::BTreeMap;

use be_block::database::{
    Database, DatabaseContent, DatabaseValue, Inverse, RowRef, inverses, short_id,
};
use be_block::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseRelationOptions, DatabaseSchema,
    DatabaseSchemaContent,
};
use be_block::database_view::{DatabaseView, DatabaseViewContent, DatabaseViewKind};
use be_block::text::{TextBlock, TextContent};
use be_block::{BlockContent, BlockMetadata, DerivedMetadata, LiveEdit, ObjectId};
use be_client::{Peer, Saved};
use be_graph::BlockParent;
use be_store::ObjectStore;
use block_ui::database::{ValueLabels, database_value_text, parse_cell_value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::blocks::{Entry, Index};
use crate::session::explain;

const ATTEMPTS: usize = 8;
const CLIENT: u64 = 0x6265_2d63_6c69;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldView {
    pub name: String,
    pub text: String,
    pub links: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    pub id: String,
    pub database: String,
    pub title: String,
    pub fields: Vec<FieldView>,
    pub page: Option<Uuid>,
    pub body: Option<String>,
}

impl RowView {
    pub fn field(&self, name: &str) -> Option<&FieldView> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    Is(String),
    IsNot(String),
    Contains(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub field: String,
    pub condition: Condition,
}

impl Filter {
    pub fn parse(text: &str) -> Result<Self, String> {
        let split = |separator: &str| {
            text.split_once(separator)
                .map(|(field, value)| (field.trim().to_owned(), value.trim().to_owned()))
        };
        let (field, condition) = if let Some((field, value)) = split("!=") {
            (field, Condition::IsNot(value))
        } else if let Some((field, value)) = split("~") {
            (field, Condition::Contains(value))
        } else if let Some((field, value)) = split("=") {
            (field, Condition::Is(value))
        } else {
            return Err(format!(
                "a filter is FIELD=VALUE, FIELD!=VALUE or FIELD~TEXT, not {text}"
            ));
        };
        Ok(Self { field, condition })
    }

    fn matches(&self, row: &RowView) -> bool {
        let (text, links) = match self.field.eq_ignore_ascii_case("title") {
            true => (row.title.clone(), Vec::new()),
            false => row
                .field(&self.field)
                .map(|field| (field.text.clone(), field.links.clone()))
                .unwrap_or_default(),
        };
        let equal = |value: &str| {
            let value = value.trim();
            (value.is_empty() && text.is_empty())
                || text.eq_ignore_ascii_case(value)
                || links.iter().any(|(id, title)| {
                    id.eq_ignore_ascii_case(value) || title.eq_ignore_ascii_case(value)
                })
        };
        match &self.condition {
            Condition::Is(value) => equal(value),
            Condition::IsNot(value) => !equal(value),
            Condition::Contains(value) => text.to_lowercase().contains(&value.to_lowercase()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Set,
    Add,
    Remove,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub field: String,
    pub change: Change,
    pub value: String,
}

impl Assignment {
    pub fn parse(text: &str) -> Result<Self, String> {
        let equals = text
            .find('=')
            .ok_or_else(|| format!("{text} is not FIELD=VALUE, FIELD+=VALUE or FIELD-=VALUE"))?;
        let (field, value) = (&text[..equals], &text[equals + 1..]);
        let (field, change) = match field.chars().last() {
            Some('+') => (&field[..field.len() - 1], Change::Add),
            Some('-') => (&field[..field.len() - 1], Change::Remove),
            _ => (field, Change::Set),
        };
        Ok(Self {
            field: field.trim().to_owned(),
            change,
            value: value.to_owned(),
        })
    }

    pub fn set(field: &str, value: &str) -> Self {
        Self {
            field: field.to_owned(),
            change: Change::Set,
            value: value.to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Keep,
    Replace(String),
    Append(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Imported {
    pub rows: usize,
    pub pages: usize,
    pub options: Vec<String>,
    pub linked_rows: Vec<String>,
    pub ignored: Vec<String>,
    pub problems: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewColumn {
    pub options: Vec<String>,
    pub links: Option<String>,
    pub multiple: bool,
    pub inverse: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Row { database: Uuid, row: ObjectId },
    Block(Entry),
}

enum Column {
    Title,
    Page,
    Field(DatabaseField),
    Ignored,
}

enum Writable {
    Own(DatabaseField),
    Inverse(Inverse),
}

struct Loaded {
    id: Uuid,
    name: String,
    database: Database,
    schema: DatabaseSchema,
    inverses: Vec<Inverse>,
}

impl Loaded {
    fn fields(&self) -> Vec<DatabaseField> {
        self.schema.fields()
    }

    fn shown_fields(&self) -> Vec<DatabaseField> {
        let mut fields = self.fields();
        fields.extend(self.inverses.iter().map(Inverse::field));
        fields
    }

    fn title_field(&self) -> Option<Uuid> {
        self.schema.title_field()
    }

    fn row(&self, row: ObjectId) -> Option<be_block::database::DatabaseRow> {
        let entry = self.database.rows.get(row)?;
        Some(
            self.inverses
                .iter()
                .fold((**entry).clone(), |shown, inverse| {
                    inverse.shown_on(&shown, row.as_uuid())
                }),
        )
    }

    fn field(&self, name: &str) -> Result<Writable, String> {
        let name = name.trim();
        if name.eq_ignore_ascii_case("title")
            && let Some(title) = self.title_field()
            && let Some(field) = self.fields().into_iter().find(|field| field.id == title)
        {
            return Ok(Writable::Own(field));
        }
        if let Some(field) = self.schema.field_named(name) {
            return Ok(Writable::Own(field));
        }
        if let Some(inverse) = self
            .inverses
            .iter()
            .find(|inverse| inverse.name.eq_ignore_ascii_case(name))
        {
            return Ok(Writable::Inverse(inverse.clone()));
        }
        Err(format!(
            "{} has no column called {name}; it has {}",
            self.name,
            self.shown_fields()
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

pub struct Tracker<S: ObjectStore> {
    peer: Peer<S>,
    index: Mutex<Option<Index>>,
}

impl<S: ObjectStore> Tracker<S> {
    pub fn new(peer: Peer<S>) -> Self {
        Self {
            peer,
            index: Mutex::new(None),
        }
    }

    pub fn peer(&self) -> &Peer<S> {
        &self.peer
    }

    pub async fn index(&self) -> Result<Index, String> {
        let mut held = self.index.lock().await;
        if let Some(index) = held.as_ref() {
            return Ok(index.clone());
        }
        let mut entries = Vec::new();
        for summary in self.peer.list_blocks().await.map_err(explain)? {
            let metadata = self.peer.metadata(&summary).unwrap_or_default();
            entries.push(Entry {
                id: summary.id,
                short: String::new(),
                block_type: summary.content_type,
                parent: summary.parent,
                name: metadata.name,
                prefix: metadata.derived.prefix,
                references: summary.references,
                access: summary.access,
            });
        }
        let index = Index::new(entries);
        *held = Some(index.clone());
        Ok(index)
    }

    pub async fn invalidate(&self) {
        *self.index.lock().await = None;
    }

    pub async fn resolve(&self, text: &str) -> Result<Target, String> {
        if looks_like_row(text) {
            let (database, row) = self.find_row(text).await?;
            return Ok(Target::Row { database, row });
        }
        let index = self.index().await?;
        match index.find(text, |_| true) {
            Ok(entry) => Ok(Target::Block(entry.clone())),
            Err(error) if is_code(text) => match self.find_row(text).await {
                Ok((database, row)) => Ok(Target::Row { database, row }),
                Err(_) => Err(error),
            },
            Err(error) => Err(error),
        }
    }

    pub async fn database_of(&self, entry: &Entry) -> Result<Uuid, String> {
        if entry.block_type == DatabaseContent::CONTENT_TYPE {
            return Ok(entry.id);
        }
        if entry.block_type == DatabaseViewContent::CONTENT_TYPE {
            let view = self
                .peer
                .open::<DatabaseViewContent>(entry.id)
                .await
                .map_err(explain)?
                .map(|content| content.root())
                .unwrap_or_default();
            return view
                .database
                .ok_or_else(|| format!("{} shows no database yet", entry.label()));
        }
        Err(format!(
            "{} ({}) is a {}, not a database",
            entry.label(),
            entry.short,
            entry.kind()
        ))
    }

    pub async fn find_database(&self, wanted: &str) -> Result<Uuid, String> {
        let index = self.index().await?;
        let entry = index
            .find(wanted, |entry| {
                entry.block_type == DatabaseContent::CONTENT_TYPE
            })?
            .clone();
        self.database_of(&entry).await
    }

    async fn prefix_of(&self, entry: &Entry) -> Result<String, String> {
        if let Some(prefix) = &entry.prefix {
            return Ok(prefix.clone());
        }
        Ok(self
            .peer
            .open::<DatabaseContent>(entry.id)
            .await
            .map_err(explain)?
            .map(|content| content.root().prefix)
            .unwrap_or_default())
    }

    async fn database_with_prefix(&self, prefix: &str) -> Result<Option<Uuid>, String> {
        let index = self.index().await?;
        let mut found = Vec::new();
        for entry in index
            .entries
            .iter()
            .filter(|entry| entry.block_type == DatabaseContent::CONTENT_TYPE)
            .filter(|entry| entry.access.can_view())
        {
            let unknown = entry.prefix.is_none();
            let known_and_different = entry
                .prefix
                .as_deref()
                .is_some_and(|held| !held.eq_ignore_ascii_case(prefix));
            if known_and_different {
                continue;
            }
            if unknown && !self.prefix_of(entry).await?.eq_ignore_ascii_case(prefix) {
                continue;
            }
            found.push(entry);
        }
        match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(one.id)),
            several => Err(format!(
                "{} databases use the row id prefix {prefix:?}: {}; give each its own prefix",
                several.len(),
                several
                    .iter()
                    .map(|entry| format!("{} ({})", entry.label(), entry.short))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    async fn find_row(&self, code: &str) -> Result<(Uuid, ObjectId), String> {
        let prefix = code
            .rsplit_once('-')
            .map_or(String::new(), |(prefix, _)| prefix.trim().to_owned());
        let database =
            self.database_with_prefix(&prefix)
                .await?
                .ok_or_else(|| match prefix.is_empty() {
                    true => format!("no row has the id {code}"),
                    false => format!("no database uses the row id prefix {prefix}"),
                })?;
        let loaded = self.load(database).await?;
        let row = loaded
            .database
            .find_short_id(code)
            .ok_or_else(|| format!("{} has no row {code}", loaded.name))?;
        Ok((database, row))
    }

    async fn load_plain(&self, id: Uuid) -> Result<(Database, DatabaseSchema), String> {
        let database = self
            .peer
            .open::<DatabaseContent>(id)
            .await
            .map_err(explain)?
            .map(|content| content.root())
            .unwrap_or_default();
        let schema = match database.schema {
            Some(schema) => self
                .peer
                .open::<DatabaseSchemaContent>(schema)
                .await
                .map_err(explain)?
                .map(|content| content.root())
                .unwrap_or_default(),
            None => DatabaseSchema::default(),
        };
        Ok((database, schema))
    }

    async fn load(&self, id: Uuid) -> Result<Loaded, String> {
        let index = self.index().await?;
        let name = index
            .get(id)
            .map_or_else(|| "Database".to_owned(), Entry::label);
        let (database, schema) = self.load_plain(id).await?;
        let mut sources: Vec<Uuid> = index
            .entries
            .iter()
            .filter(|entry| entry.block_type == DatabaseSchemaContent::CONTENT_TYPE)
            .filter(|entry| entry.references.contains(&id))
            .filter_map(|entry| entry.parent.block())
            .collect();
        sources.sort();
        sources.dedup();
        let mut found = Vec::new();
        for source in sources {
            let held;
            let (source_database, source_schema) = match source == id {
                true => (&database, &schema),
                false => {
                    held = match self.load_plain(source).await {
                        Ok(held) => held,
                        Err(_) => continue,
                    };
                    (&held.0, &held.1)
                }
            };
            found.extend(inverses(id, source, source_database, source_schema));
        }
        Ok(Loaded {
            id,
            name,
            database,
            schema,
            inverses: found,
        })
    }

    async fn labels(&self, loaded: &Loaded) -> Result<ValueLabels, String> {
        let mut labels = ValueLabels::new();
        let mut targets: Vec<Uuid> = loaded
            .shown_fields()
            .iter()
            .filter(|field| field.field_type == DatabaseFieldType::Relation)
            .filter_map(|field| field.relation_options.database)
            .collect();
        targets.sort();
        targets.dedup();
        for target in targets {
            let (database, schema) = match target == loaded.id {
                true => (loaded.database.clone(), loaded.schema.clone()),
                false => match self.load_plain(target).await {
                    Ok(held) => held,
                    Err(_) => continue,
                },
            };
            let title = schema.title_field();
            for row in database.rows.iter() {
                labels.rows.insert(
                    RowRef {
                        database: target,
                        row: row.id.as_uuid(),
                    },
                    format!(
                        "{} {}",
                        database.short_id(row.id),
                        database.title(row.id, title)
                    ),
                );
            }
        }
        Ok(labels)
    }

    fn view(&self, loaded: &Loaded, row: ObjectId, labels: &ValueLabels) -> RowView {
        let title_field = loaded.title_field();
        let entry = loaded.row(row);
        let fields = loaded
            .shown_fields()
            .into_iter()
            .filter(|field| Some(field.id) != title_field)
            .filter_map(|field| {
                let value = entry.as_ref()?.get(field.id)?;
                let links = match &value {
                    DatabaseValue::Links(links) => links.clone(),
                    DatabaseValue::Relation(link) => vec![*link],
                    _ => Vec::new(),
                }
                .iter()
                .map(|link| {
                    let shown = labels.row(link);
                    match shown.split_once(' ') {
                        Some((id, title)) => (id.to_owned(), title.to_owned()),
                        None => (shown, String::new()),
                    }
                })
                .collect();
                Some(FieldView {
                    name: field.name.clone(),
                    text: database_value_text(&value, &field, labels),
                    links,
                })
            })
            .collect();
        RowView {
            id: loaded.database.short_id(row),
            database: loaded.name.clone(),
            title: loaded.database.title(row, title_field),
            fields,
            page: entry.and_then(|entry| entry.page()),
            body: None,
        }
    }

    pub async fn databases(&self) -> Result<Vec<Entry>, String> {
        let index = self.index().await?;
        let mut found = Vec::new();
        for entry in index
            .entries
            .iter()
            .filter(|entry| entry.block_type == DatabaseContent::CONTENT_TYPE)
            .filter(|entry| entry.access.can_view())
        {
            let mut entry = entry.clone();
            entry.prefix = Some(self.prefix_of(&entry).await?);
            found.push(entry);
        }
        Ok(found)
    }

    pub async fn list(&self, database: Uuid, filters: &[Filter]) -> Result<Vec<RowView>, String> {
        let loaded = self.load(database).await?;
        let labels = self.labels(&loaded).await?;
        Ok(loaded
            .database
            .rows
            .iter()
            .map(|row| self.view(&loaded, row.id, &labels))
            .filter(|row| filters.iter().all(|filter| filter.matches(row)))
            .collect())
    }

    pub async fn show(&self, code: &str) -> Result<RowView, String> {
        let (database, row) = self.find_row(code).await?;
        self.show_row(database, row).await
    }

    async fn show_row(&self, database: Uuid, row: ObjectId) -> Result<RowView, String> {
        let loaded = self.load(database).await?;
        let labels = self.labels(&loaded).await?;
        let mut view = self.view(&loaded, row, &labels);
        if let Some(page) = view.page {
            view.body = Some(self.text(page).await?);
        }
        Ok(view)
    }

    pub async fn schema(&self, database: Uuid) -> Result<Vec<String>, String> {
        let loaded = self.load(database).await?;
        let index = self.index().await?;
        let title = loaded.title_field();
        let name_of = |id: Uuid| {
            index.get(id).map_or_else(
                || id.to_string(),
                |entry| format!("{} ({})", entry.label(), entry.short),
            )
        };
        let mut lines = Vec::new();
        for field in loaded.fields() {
            let kind = match field.field_type {
                DatabaseFieldType::String if Some(field.id) == title => {
                    "text, the title".to_owned()
                }
                DatabaseFieldType::String => "text".to_owned(),
                DatabaseFieldType::Number => "number".to_owned(),
                DatabaseFieldType::Boolean => "boolean".to_owned(),
                DatabaseFieldType::Color => "color".to_owned(),
                DatabaseFieldType::Datetime => "date".to_owned(),
                DatabaseFieldType::Block => "block".to_owned(),
                DatabaseFieldType::Enum => format!(
                    "one of {}",
                    field
                        .enum_options
                        .iter()
                        .map(|option| option.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                DatabaseFieldType::Relation => {
                    let options = &field.relation_options;
                    let target = options
                        .database
                        .map_or_else(|| "no database yet".to_owned(), name_of);
                    let many = match options.multiple {
                        true => "rows",
                        false => "a row",
                    };
                    match options.inverse.trim() {
                        "" => format!("{many} of {target}"),
                        inverse => format!("{many} of {target}, shown there as {inverse}"),
                    }
                }
            };
            lines.push(format!("{}\t{kind}", field.name));
        }
        for inverse in &loaded.inverses {
            let source = self
                .load_plain(inverse.source)
                .await
                .ok()
                .and_then(|(_, schema)| {
                    schema
                        .fields()
                        .into_iter()
                        .find(|field| field.id == inverse.field)
                        .map(|field| field.name)
                })
                .unwrap_or_default();
            lines.push(format!(
                "{}\trows of {} whose {source} links here",
                inverse.name,
                name_of(inverse.source)
            ));
        }
        Ok(lines)
    }

    pub async fn create_database(
        &self,
        name: &str,
        prefix: &str,
        parent: BlockParent,
    ) -> Result<(Uuid, Uuid), String> {
        let prefix = prefix.trim().to_ascii_uppercase();
        if !prefix.is_empty() && self.database_with_prefix(&prefix).await?.is_some() {
            return Err(format!("another database already uses the prefix {prefix}"));
        }
        let (view, database, schema) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let named = BlockMetadata::named(name);
        self.peer
            .create_block(view, DatabaseViewContent::CONTENT_TYPE, parent, &named)
            .await
            .map_err(explain)?;
        let mut shown = DatabaseView::of(database);
        shown.kind = DatabaseViewKind::List;
        self.peer
            .save(view, &DatabaseViewContent::new(&shown), None)
            .await
            .map_err(explain)?;
        let mut held = BlockMetadata::named(name);
        held.derived = DerivedMetadata {
            prefix: Some(prefix.clone()),
            ..DerivedMetadata::default()
        };
        self.peer
            .create_block(
                database,
                DatabaseContent::CONTENT_TYPE,
                BlockParent::Block(view),
                &held,
            )
            .await
            .map_err(explain)?;
        self.peer
            .create_block(
                schema,
                DatabaseSchemaContent::CONTENT_TYPE,
                BlockParent::Block(database),
                &BlockMetadata::default(),
            )
            .await
            .map_err(explain)?;
        let mut columns = DatabaseSchemaContent::default();
        columns.apply(&DatabaseSchema::add_field("Title", DatabaseFieldType::String).1);
        self.peer
            .save(schema, &columns, None)
            .await
            .map_err(explain)?;
        let mut content = DatabaseContent::new(&Database::with_schema(schema));
        content.apply(&Database::set_prefix(prefix));
        self.peer
            .save(database, &content, None)
            .await
            .map_err(explain)?;
        self.invalidate().await;
        Ok((view, database))
    }

    pub async fn add_column(
        &self,
        database: Uuid,
        name: &str,
        kind: &str,
        column: &NewColumn,
    ) -> Result<(), String> {
        let loaded = self.load(database).await?;
        if loaded.field(name).is_ok() {
            return Err(format!(
                "{} already has a column called {name}",
                loaded.name
            ));
        }
        let schema = loaded
            .database
            .schema
            .ok_or_else(|| format!("{} has no schema", loaded.name))?;
        let field_type = match kind.trim().to_ascii_lowercase().as_str() {
            "text" | "string" => DatabaseFieldType::String,
            "number" => DatabaseFieldType::Number,
            "enum" | "select" | "option" | "status" => DatabaseFieldType::Enum,
            "bool" | "boolean" | "checkbox" => DatabaseFieldType::Boolean,
            "date" | "datetime" => DatabaseFieldType::Datetime,
            "color" | "colour" => DatabaseFieldType::Color,
            "block" => DatabaseFieldType::Block,
            "relation" | "link" | "links" => DatabaseFieldType::Relation,
            other => {
                return Err(format!(
                    "{other} is not a column type; use text, number, enum, boolean, date, color, block or relation"
                ));
            }
        };
        let (id, add) = DatabaseSchema::add_field(name.trim(), field_type);
        let mut edits = vec![add];
        for option in &column.options {
            edits.push(DatabaseSchema::add_enum_option(id, option.trim()).1);
        }
        if field_type == DatabaseFieldType::Relation {
            let target = match &column.links {
                Some(target) => self.find_database(target).await?,
                None => {
                    return Err("a relation column needs --links DATABASE".to_owned());
                }
            };
            edits.push(DatabaseSchema::set_relation_options(
                id,
                DatabaseRelationOptions {
                    database: Some(target),
                    multiple: column.multiple,
                    inverse: column.inverse.trim().to_owned(),
                },
            ));
        }
        self.change::<DatabaseSchemaContent>(schema, |_| Ok(edits.clone()))
            .await?;
        self.invalidate().await;
        Ok(())
    }

    pub async fn create(
        &self,
        database: Uuid,
        title: &str,
        assignments: &[Assignment],
        body: Body,
    ) -> Result<RowView, String> {
        let loaded = self.load(database).await?;
        let title_field = loaded
            .title_field()
            .ok_or_else(|| format!("{} has no text column to hold a title", loaded.name))?;
        let (row, add) =
            Database::add_row([(title_field, DatabaseValue::String(title.to_owned()))]);
        self.change::<DatabaseContent>(database, |_| Ok(vec![add.clone()]))
            .await?;
        self.apply(database, row, assignments, body).await
    }

    pub async fn update(
        &self,
        code: &str,
        assignments: &[Assignment],
        body: Body,
    ) -> Result<RowView, String> {
        let (database, row) = self.find_row(code).await?;
        self.apply(database, row, assignments, body).await
    }

    async fn apply(
        &self,
        database: Uuid,
        row: ObjectId,
        assignments: &[Assignment],
        body: Body,
    ) -> Result<RowView, String> {
        let loaded = self.load(database).await?;
        let labels = self.labels(&loaded).await?;
        let code = loaded.database.short_id(row);
        let current = loaded
            .row(row)
            .ok_or_else(|| format!("{code} was deleted"))?;
        let target = RowRef {
            database,
            row: row.as_uuid(),
        };
        let mut own = Vec::new();
        let mut elsewhere: Vec<(Uuid, be_block::Edit)> = Vec::new();
        let mut retitled = None;
        for assignment in assignments {
            match loaded.field(&assignment.field)? {
                Writable::Own(field) => {
                    if Some(field.id) == loaded.title_field() {
                        retitled = Some(assignment.value.clone());
                    }
                    own.push(self.assigned(&loaded, row, &current, &field, assignment, &labels)?);
                }
                Writable::Inverse(inverse) => {
                    let field = inverse.field();
                    let wanted = self.wanted_links(&current, &field, assignment, &labels)?;
                    let source = match inverse.source == database {
                        true => loaded.database.clone(),
                        false => self.load_plain(inverse.source).await?.0,
                    };
                    let edit = inverse.edits(&source, target, &wanted);
                    match inverse.source == database {
                        true => own.push(edit),
                        false => elsewhere.push((inverse.source, edit)),
                    }
                }
            }
        }
        if !own.is_empty() {
            self.change::<DatabaseContent>(database, |current| {
                if current.root().rows.get(row).is_none() {
                    return Err(format!("{code} was deleted"));
                }
                Ok(own.clone())
            })
            .await?;
        }
        for (source, edit) in elsewhere {
            self.change::<DatabaseContent>(source, |_| Ok(vec![edit.clone()]))
                .await?;
        }
        let page = loaded.database.rows.get(row).and_then(|row| row.page());
        if let (Some(title), Some(page)) = (&retitled, page) {
            self.peer
                .set_metadata(page, &BlockMetadata::named(title.clone()))
                .await
                .map_err(explain)?;
        }
        if body != Body::Keep {
            let title =
                retitled.unwrap_or_else(|| loaded.database.title(row, loaded.title_field()));
            self.write_page(database, row, &title, body).await?;
        }
        self.show_row(database, row).await
    }

    fn assigned(
        &self,
        loaded: &Loaded,
        row: ObjectId,
        current: &be_block::database::DatabaseRow,
        field: &DatabaseField,
        assignment: &Assignment,
        labels: &ValueLabels,
    ) -> Result<be_block::Edit, String> {
        if field.field_type == DatabaseFieldType::Relation && field.relation_options.multiple {
            let wanted = self.wanted_links(current, field, assignment, labels)?;
            return Ok(loaded.database.set_links(row, field.id, &wanted));
        }
        let value = match assignment.change {
            Change::Set => self.value(field, &assignment.value, labels)?,
            Change::Remove => {
                let removed = self.value(field, &assignment.value, labels)?;
                match current.value(field.id) == removed.as_ref() {
                    true => None,
                    false => current.value(field.id).cloned(),
                }
            }
            Change::Add => {
                return Err(format!(
                    "{} holds one value; use {}=VALUE",
                    field.name, field.name
                ));
            }
        };
        Ok(Database::set_value(row, field.id, value.as_ref()))
    }

    fn wanted_links(
        &self,
        current: &be_block::database::DatabaseRow,
        field: &DatabaseField,
        assignment: &Assignment,
        labels: &ValueLabels,
    ) -> Result<Vec<RowRef>, String> {
        let named = assignment
            .value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| self.link(field, item, labels))
            .collect::<Result<Vec<_>, _>>()?;
        let mut links = match current.get(field.id) {
            Some(DatabaseValue::Links(links)) => links,
            Some(DatabaseValue::Relation(link)) => vec![link],
            _ => Vec::new(),
        };
        match assignment.change {
            Change::Set => links = named,
            Change::Add => links.extend(
                named
                    .into_iter()
                    .filter(|link| !links.contains(link))
                    .collect::<Vec<_>>(),
            ),
            Change::Remove => links.retain(|link| !named.contains(link)),
        }
        Ok(links)
    }

    fn link(
        &self,
        field: &DatabaseField,
        wanted: &str,
        labels: &ValueLabels,
    ) -> Result<RowRef, String> {
        let database = field
            .relation_options
            .database
            .ok_or_else(|| format!("{} does not link to a database yet", field.name))?;
        let code = wanted.rsplit('-').next().unwrap_or(wanted);
        let rows = labels.rows_of(database);
        rows.iter()
            .find(|(link, _)| short_id("", link.row).eq_ignore_ascii_case(code) && code.len() == 6)
            .or_else(|| {
                rows.iter().find(|(_, label)| {
                    label
                        .split_once(' ')
                        .is_some_and(|(_, title)| title.eq_ignore_ascii_case(wanted))
                })
            })
            .map(|(link, _)| *link)
            .ok_or_else(|| format!("{} has no row {wanted} to link", field.name))
    }

    pub async fn delete(&self, code: &str) -> Result<String, String> {
        let (database, row) = self.find_row(code).await?;
        let loaded = self.load(database).await?;
        let page = loaded.database.rows.get(row).and_then(|row| row.page());
        let title = loaded.database.title(row, loaded.title_field());
        self.change::<DatabaseContent>(database, |current| {
            Ok(match current.root().rows.get(row) {
                Some(_) => vec![Database::remove_row(row)],
                None => Vec::new(),
            })
        })
        .await?;
        if let Some(page) = page {
            self.peer
                .set_parent(page, BlockParent::Detached)
                .await
                .map_err(explain)?;
            self.invalidate().await;
        }
        Ok(format!("deleted {} {title}", loaded.database.short_id(row)))
    }

    pub async fn page_of(&self, code: &str, create: bool) -> Result<Option<Uuid>, String> {
        let (database, row) = self.find_row(code).await?;
        let loaded = self.load(database).await?;
        let page = loaded.database.rows.get(row).and_then(|row| row.page());
        if page.is_some() || !create {
            return Ok(page);
        }
        let title = loaded.database.title(row, loaded.title_field());
        self.write_page(database, row, &title, Body::Replace(String::new()))
            .await?;
        Ok(self
            .load(database)
            .await?
            .database
            .rows
            .get(row)
            .and_then(|row| row.page()))
    }

    pub async fn text(&self, block: Uuid) -> Result<String, String> {
        Ok(self
            .peer
            .open::<TextContent>(block)
            .await
            .map_err(explain)?
            .map(|content| TextBlock::text(&content))
            .unwrap_or_default())
    }

    pub async fn write_text(&self, block: Uuid, body: &Body) -> Result<(), String> {
        self.change::<TextContent>(block, |current| {
            let edit = match body {
                Body::Keep => None,
                Body::Replace(text) => replace_text(current, text),
                Body::Append(text) => {
                    let now = TextBlock::text(current);
                    let joined = match now.is_empty() || now.ends_with('\n') {
                        true => text.clone(),
                        false => format!("\n{text}"),
                    };
                    TextBlock::insert(current, CLIENT, now.len(), joined.as_bytes())
                }
            };
            Ok(edit.into_iter().collect())
        })
        .await?;
        Ok(())
    }

    pub async fn rewrite_text(
        &self,
        block: Uuid,
        base: &str,
        edited: &str,
    ) -> Result<bool, String> {
        let mut clean = true;
        self.change::<TextContent>(block, |current| {
            let now = TextBlock::text(current);
            let wanted = match now == base {
                true => edited.to_owned(),
                false => {
                    let outcome = be_commit::merge::merge_lines(
                        base.as_bytes(),
                        edited.as_bytes(),
                        now.as_bytes(),
                    );
                    clean = outcome.is_clean();
                    String::from_utf8_lossy(&be_commit::merge::render_conflicts(
                        &outcome, "yours", "theirs",
                    ))
                    .into_owned()
                }
            };
            Ok(replace_text(current, &wanted).into_iter().collect())
        })
        .await?;
        Ok(clean)
    }

    pub async fn import(
        &self,
        database: Uuid,
        records: Vec<Vec<String>>,
        page_column: &str,
    ) -> Result<Imported, String> {
        let mut records = records.into_iter();
        let header = records.next().ok_or("the file has no header row")?;
        let rows: Vec<Vec<String>> = records
            .filter(|record| record.iter().any(|cell| !cell.trim().is_empty()))
            .collect();
        let mut loaded = self.load(database).await?;
        let mut imported = Imported::default();
        let schema = loaded
            .database
            .schema
            .ok_or_else(|| format!("{} has no schema", loaded.name))?;
        let column_values = |index: usize| -> Vec<String> {
            let mut values: Vec<String> = Vec::new();
            for row in &rows {
                let cell = row.get(index).map(|cell| cell.trim()).unwrap_or_default();
                if !cell.is_empty() && !values.iter().any(|held| held.eq_ignore_ascii_case(cell)) {
                    values.push(cell.to_owned());
                }
            }
            values
        };
        let mut additions = Vec::new();
        for (index, name) in header.iter().enumerate() {
            let Some(field) = loaded.schema.field_named(name) else {
                continue;
            };
            match field.field_type {
                DatabaseFieldType::Enum => {
                    for option in column_values(index) {
                        if field
                            .enum_options
                            .iter()
                            .any(|known| known.name.eq_ignore_ascii_case(&option))
                        {
                            continue;
                        }
                        imported.options.push(format!("{}: {option}", field.name));
                        additions.push(DatabaseSchema::add_enum_option(field.id, option).1);
                    }
                }
                DatabaseFieldType::Relation => {
                    let Some(target) = field.relation_options.database else {
                        continue;
                    };
                    if target == database {
                        continue;
                    }
                    let mut wanted: Vec<String> = column_values(index);
                    if field.relation_options.multiple {
                        wanted = wanted
                            .iter()
                            .flat_map(|cell| cell.split(',').map(|item| item.trim().to_owned()))
                            .filter(|item| !item.is_empty())
                            .collect();
                        wanted.sort();
                        wanted.dedup();
                    }
                    let labels = self.labels(&loaded).await?;
                    let missing: Vec<String> = wanted
                        .into_iter()
                        .filter(|title| self.link(&field, title, &labels).is_err())
                        .collect();
                    if missing.is_empty() {
                        continue;
                    }
                    let (_, target_schema) = self.load_plain(target).await?;
                    let title = target_schema.title_field().ok_or_else(|| {
                        format!("the database {} links to has no title column", field.name)
                    })?;
                    let adds: Vec<be_block::Edit> = missing
                        .iter()
                        .map(|name| {
                            Database::add_row([(title, DatabaseValue::String(name.clone()))]).1
                        })
                        .collect();
                    self.change::<DatabaseContent>(target, |_| Ok(adds.clone()))
                        .await?;
                    imported.linked_rows.extend(
                        missing
                            .into_iter()
                            .map(|name| format!("{}: {name}", field.name)),
                    );
                }
                _ => {}
            }
        }
        if !additions.is_empty() {
            self.change::<DatabaseSchemaContent>(schema, |_| Ok(additions.clone()))
                .await?;
        }
        loaded = self.load(database).await?;
        let title_field = loaded
            .title_field()
            .ok_or_else(|| format!("{} has no text column to hold a title", loaded.name))?;
        let columns: Vec<Column> = header
            .iter()
            .map(|name| {
                if name.trim().eq_ignore_ascii_case(page_column) {
                    return Column::Page;
                }
                match loaded.field(name) {
                    Ok(Writable::Own(field)) if field.id == title_field => Column::Title,
                    Ok(Writable::Own(field)) => Column::Field(field),
                    _ => {
                        imported.ignored.push(name.clone());
                        Column::Ignored
                    }
                }
            })
            .collect();
        if !columns.iter().any(|column| matches!(column, Column::Title)) {
            return Err(format!(
                "the file needs a Title column, or one named like {}'s first text column",
                loaded.name
            ));
        }
        let labels = self.labels(&loaded).await?;
        let mut edits = Vec::new();
        for (number, row) in rows.iter().enumerate() {
            let mut values = BTreeMap::new();
            let mut links = Vec::new();
            let mut title = String::new();
            let mut page_text = String::new();
            for (column, cell) in columns.iter().zip(row) {
                match column {
                    Column::Title => title = cell.trim().to_owned(),
                    Column::Page => page_text = cell.clone(),
                    Column::Ignored => {}
                    Column::Field(field)
                        if field.field_type == DatabaseFieldType::Relation
                            && field.relation_options.multiple =>
                    {
                        for item in cell
                            .split(',')
                            .map(str::trim)
                            .filter(|item| !item.is_empty())
                        {
                            match self.link(field, item, &labels) {
                                Ok(link) => links.push((field.id, link)),
                                Err(error) => imported.problems.push(format!(
                                    "row {}, {}: {error}",
                                    number + 2,
                                    field.name
                                )),
                            }
                        }
                    }
                    Column::Field(field) => match self.value(field, cell, &labels) {
                        Ok(Some(value)) => {
                            values.insert(field.id, value);
                        }
                        Ok(None) => {}
                        Err(error) => imported.problems.push(format!(
                            "row {}, {}: {error}",
                            number + 2,
                            field.name
                        )),
                    },
                }
            }
            values.insert(title_field, DatabaseValue::String(title.clone()));
            let (id, add) = Database::add_row(values);
            edits.push(add);
            edits.extend(
                links
                    .into_iter()
                    .map(|(field, link)| Database::link(id, field, link)),
            );
            if !page_text.trim().is_empty() {
                let page = Uuid::new_v4();
                self.peer
                    .create_block(
                        page,
                        TextContent::CONTENT_TYPE,
                        BlockParent::Block(database),
                        &BlockMetadata::named(title),
                    )
                    .await
                    .map_err(explain)?;
                self.peer
                    .save(page, &TextBlock::new(page_text).content(), None)
                    .await
                    .map_err(explain)?;
                edits.push(Database::set_page(id, Some(page)));
                imported.pages += 1;
            }
            imported.rows += 1;
        }
        self.change::<DatabaseContent>(database, |_| Ok(edits.clone()))
            .await?;
        self.invalidate().await;
        Ok(imported)
    }

    async fn write_page(
        &self,
        database: Uuid,
        row: ObjectId,
        title: &str,
        body: Body,
    ) -> Result<(), String> {
        let (current, _) = self.load_plain(database).await?;
        let page = match current.rows.get(row).and_then(|row| row.page()) {
            Some(page) => page,
            None => {
                let page = Uuid::new_v4();
                self.peer
                    .create_block(
                        page,
                        TextContent::CONTENT_TYPE,
                        BlockParent::Block(database),
                        &BlockMetadata::named(title),
                    )
                    .await
                    .map_err(explain)?;
                self.peer
                    .save(page, &TextBlock::new("").content(), None)
                    .await
                    .map_err(explain)?;
                self.change::<DatabaseContent>(database, |_| {
                    Ok(vec![Database::set_page(row, Some(page))])
                })
                .await?;
                self.invalidate().await;
                page
            }
        };
        self.write_text(page, &body).await
    }

    fn value(
        &self,
        field: &DatabaseField,
        text: &str,
        labels: &ValueLabels,
    ) -> Result<Option<DatabaseValue>, String> {
        let wanted = text.trim();
        if wanted.is_empty() {
            return Ok(None);
        }
        let value = match field.field_type {
            DatabaseFieldType::Enum => field
                .enum_options
                .iter()
                .find(|option| option.name.eq_ignore_ascii_case(wanted))
                .map(|option| DatabaseValue::Enum(option.id)),
            DatabaseFieldType::Relation => {
                return self
                    .link(field, wanted, labels)
                    .map(|link| Some(DatabaseValue::Relation(link)));
            }
            DatabaseFieldType::Block => Uuid::parse_str(wanted).ok().map(DatabaseValue::Block),
            _ => parse_cell_value(wanted, field),
        };
        match value {
            Some(value) => Ok(Some(value)),
            None => Err(match field.field_type {
                DatabaseFieldType::Enum => format!(
                    "{} is not one of {}: {}",
                    wanted,
                    field.name,
                    field
                        .enum_options
                        .iter()
                        .map(|option| option.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                _ => format!("{wanted} is not a value {} can hold", field.name),
            }),
        }
    }

    async fn change<C>(
        &self,
        block: Uuid,
        mut make: impl FnMut(&C) -> Result<Vec<C::Op>, String>,
    ) -> Result<C, String>
    where
        C: BlockContent + LiveEdit + Default,
    {
        for _ in 0..ATTEMPTS {
            let opened = self.peer.open::<C>(block).await.map_err(explain)?;
            let head = opened.as_ref().and_then(|_| self.peer.local_head(block));
            let mut content = opened.unwrap_or_default();
            let edits = make(&content)?;
            if edits.is_empty() {
                return Ok(content);
            }
            for edit in &edits {
                content.apply(edit);
            }
            match self
                .peer
                .save(block, &content, head)
                .await
                .map_err(explain)?
            {
                Saved::Rejected { .. } => continue,
                Saved::Published(_) | Saved::Unchanged(_) => return Ok(content),
            }
        }
        Err(format!(
            "block {block} kept changing while this was being saved; try again"
        ))
    }
}

pub fn looks_like_row(text: &str) -> bool {
    let Some((prefix, code)) = text.trim().rsplit_once('-') else {
        return false;
    };
    !prefix.is_empty()
        && prefix
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && prefix
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && is_code(code)
}

fn is_code(text: &str) -> bool {
    text.len() == 6
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn replace_text(current: &TextContent, text: &str) -> Option<be_block::Edit> {
    let now = TextBlock::text(current);
    if now == text {
        return None;
    }
    let start = now
        .bytes()
        .zip(text.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let start = (0..=start)
        .rev()
        .find(|index| now.is_char_boundary(*index) && text.is_char_boundary(*index))
        .unwrap_or(0);
    let tail = now[start..]
        .bytes()
        .rev()
        .zip(text[start..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let tail = (0..=tail)
        .rev()
        .find(|length| {
            now.is_char_boundary(now.len() - length) && text.is_char_boundary(text.len() - length)
        })
        .unwrap_or(0);
    TextBlock::replace(
        current,
        CLIENT,
        start..now.len() - tail,
        &text.as_bytes()[start..text.len() - tail],
    )
}
