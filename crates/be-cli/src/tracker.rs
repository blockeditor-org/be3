use std::collections::BTreeMap;

use be_block::database::{Database, DatabaseContent, DatabaseValue, RowRef};
use be_block::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaContent,
};
use be_block::text::{TextBlock, TextContent};
use be_block::{BlockContent, BlockMetadata, LiveEdit, ObjectId};
use be_client::{Peer, Saved};
use be_graph::BlockParent;
use be_store::ObjectStore;
use block_ui::database::{ValueLabels, database_value_text, parse_cell_value};
use uuid::Uuid;

use crate::session::explain;

const ATTEMPTS: usize = 8;
const CLIENT: u64 = 0x6265_2d63_6c69;

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseInfo {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub rows: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    pub id: String,
    pub database: String,
    pub title: String,
    pub fields: Vec<(String, String)>,
    pub page: Option<Uuid>,
    pub body: Option<String>,
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
        let shown = match self.field.eq_ignore_ascii_case("title") {
            true => Some(row.title.clone()),
            false => row
                .fields
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&self.field))
                .map(|(_, value)| value.clone()),
        }
        .unwrap_or_default();
        match &self.condition {
            Condition::Is(value) => shown.eq_ignore_ascii_case(value),
            Condition::IsNot(value) => !shown.eq_ignore_ascii_case(value),
            Condition::Contains(value) => shown.to_lowercase().contains(&value.to_lowercase()),
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
    pub ignored: Vec<String>,
    pub problems: Vec<String>,
}

enum Column {
    Title,
    Page,
    Field(DatabaseField),
    Ignored,
}

struct Loaded {
    id: Uuid,
    name: String,
    database: Database,
    schema: DatabaseSchema,
}

impl Loaded {
    fn fields(&self) -> Vec<DatabaseField> {
        self.schema.fields()
    }

    fn title_field(&self) -> Option<Uuid> {
        self.schema.title_field()
    }

    fn field(&self, name: &str) -> Result<DatabaseField, String> {
        if name.eq_ignore_ascii_case("title")
            && let Some(title) = self.title_field()
        {
            return self
                .fields()
                .into_iter()
                .find(|field| field.id == title)
                .ok_or_else(|| "the title column is gone".to_owned());
        }
        self.schema.field_named(name).ok_or_else(|| {
            format!(
                "{} has no column called {name}; it has {}",
                self.name,
                self.fields()
                    .iter()
                    .map(|field| field.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }
}

pub struct Tracker<S: ObjectStore> {
    peer: Peer<S>,
}

impl<S: ObjectStore> Tracker<S> {
    pub fn new(peer: Peer<S>) -> Self {
        Self { peer }
    }

    pub fn peer(&self) -> &Peer<S> {
        &self.peer
    }

    pub async fn databases(&self) -> Result<Vec<DatabaseInfo>, String> {
        let mut found = Vec::new();
        for summary in self.peer.list_blocks().await.map_err(explain)? {
            if summary.content_type != DatabaseContent::CONTENT_TYPE || !summary.access.can_view() {
                continue;
            }
            let name = self
                .peer
                .metadata(&summary)
                .ok()
                .and_then(|metadata| metadata.name)
                .unwrap_or_else(|| "Database".to_owned());
            let database = self
                .peer
                .open::<DatabaseContent>(summary.id)
                .await
                .map_err(explain)?
                .map(|content| content.root())
                .unwrap_or_default();
            found.push(DatabaseInfo {
                id: summary.id,
                name,
                prefix: database.prefix.clone(),
                rows: database.rows.len(),
            });
        }
        found.sort_by_key(|a| a.name.to_lowercase());
        Ok(found)
    }

    pub async fn find_database(&self, wanted: &str) -> Result<Uuid, String> {
        let databases = self.databases().await?;
        if let Ok(id) = Uuid::parse_str(wanted)
            && databases.iter().any(|database| database.id == id)
        {
            return Ok(id);
        }
        let matches: Vec<&DatabaseInfo> = databases
            .iter()
            .filter(|database| {
                database.name.eq_ignore_ascii_case(wanted)
                    || (!database.prefix.is_empty() && database.prefix.eq_ignore_ascii_case(wanted))
            })
            .collect();
        match matches.as_slice() {
            [one] => Ok(one.id),
            [] => Err(format!(
                "no database is called {wanted}; there are {}",
                databases
                    .iter()
                    .map(|database| database.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            _ => Err(format!(
                "more than one database is called {wanted}; use its id"
            )),
        }
    }

    async fn load(&self, id: Uuid) -> Result<Loaded, String> {
        let summary = self.peer.summary(id).await.map_err(explain)?;
        let name = self
            .peer
            .metadata(&summary)
            .ok()
            .and_then(|metadata| metadata.name)
            .unwrap_or_else(|| "Database".to_owned());
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
        Ok(Loaded {
            id,
            name,
            database,
            schema,
        })
    }

    async fn find_row(&self, code: &str) -> Result<(Loaded, ObjectId), String> {
        let prefix = code
            .rsplit_once('-')
            .map(|(prefix, _)| prefix.trim().to_owned());
        for database in self.databases().await? {
            if let Some(prefix) = &prefix
                && !database.prefix.eq_ignore_ascii_case(prefix)
            {
                continue;
            }
            let loaded = self.load(database.id).await?;
            if let Some(row) = loaded.database.find_short_id(code) {
                return Ok((loaded, row));
            }
        }
        Err(format!("no row has the id {code}"))
    }

    async fn labels(&self, loaded: &Loaded) -> Result<ValueLabels, String> {
        let mut labels = ValueLabels::new();
        let mut targets: Vec<Uuid> = loaded
            .fields()
            .iter()
            .filter(|field| field.field_type == DatabaseFieldType::Relation)
            .filter_map(|field| field.relation_options.database)
            .collect();
        targets.sort();
        targets.dedup();
        for target in targets {
            let Ok(linked) = self.load(target).await else {
                continue;
            };
            let title = linked.title_field();
            for row in linked.database.rows.iter() {
                labels.rows.insert(
                    RowRef {
                        database: target,
                        row: row.id.as_uuid(),
                    },
                    linked.database.title(row.id, title),
                );
            }
        }
        Ok(labels)
    }

    fn view(&self, loaded: &Loaded, row: ObjectId, labels: &ValueLabels) -> RowView {
        let title_field = loaded.title_field();
        let entry = loaded.database.rows.get(row);
        let fields = loaded
            .fields()
            .into_iter()
            .filter(|field| Some(field.id) != title_field)
            .filter_map(|field| {
                let value = entry?.value(field.id)?;
                Some((
                    field.name.clone(),
                    database_value_text(value, &field, labels),
                ))
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
        let (loaded, row) = self.find_row(code).await?;
        let labels = self.labels(&loaded).await?;
        let mut view = self.view(&loaded, row, &labels);
        if let Some(page) = view.page {
            view.body = Some(self.page_text(page).await?);
        }
        Ok(view)
    }

    pub async fn page_text(&self, page: Uuid) -> Result<String, String> {
        Ok(self
            .peer
            .open::<TextContent>(page)
            .await
            .map_err(explain)?
            .map(|content| TextBlock::text(&content))
            .unwrap_or_default())
    }

    pub async fn create(
        &self,
        database: Uuid,
        title: &str,
        sets: &[(String, String)],
        body: Body,
    ) -> Result<RowView, String> {
        let loaded = self.load(database).await?;
        let title_field = loaded
            .title_field()
            .ok_or_else(|| format!("{} has no text column to hold a title", loaded.name))?;
        let labels = self.labels(&loaded).await?;
        let mut values = BTreeMap::from([(title_field, DatabaseValue::String(title.to_owned()))]);
        for (name, text) in sets {
            let field = loaded.field(name)?;
            match self.value(&field, text, &labels)? {
                Some(value) => values.insert(field.id, value),
                None => values.remove(&field.id),
            };
        }
        let (row, add) = Database::add_row(values);
        self.change::<DatabaseContent>(database, |_| Ok(vec![add.clone()]))
            .await?;
        if body != Body::Keep {
            self.write_page(&loaded, row, title, body).await?;
        }
        self.show(&loaded.database.short_id(row)).await
    }

    pub async fn update(
        &self,
        code: &str,
        sets: &[(String, String)],
        body: Body,
    ) -> Result<RowView, String> {
        let (loaded, row) = self.find_row(code).await?;
        let labels = self.labels(&loaded).await?;
        let mut edits = Vec::new();
        let mut retitled = None;
        for (name, text) in sets {
            let field = loaded.field(name)?;
            let value = self.value(&field, text, &labels)?;
            if Some(field.id) == loaded.title_field() {
                retitled = Some(text.clone());
            }
            edits.push(Database::set_value(row, field.id, value.as_ref()));
        }
        if !edits.is_empty() {
            self.change::<DatabaseContent>(loaded.id, |current| {
                if current.root().rows.get(row).is_none() {
                    return Err(format!("{code} was deleted"));
                }
                Ok(edits.clone())
            })
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
            self.write_page(&loaded, row, &title, body).await?;
        }
        self.show(code).await
    }

    pub async fn rewrite_page(&self, code: &str, base: &str, edited: &str) -> Result<bool, String> {
        let (loaded, row) = self.find_row(code).await?;
        let Some(page) = loaded.database.rows.get(row).and_then(|row| row.page()) else {
            if edited.trim().is_empty() {
                return Ok(true);
            }
            let title = loaded.database.title(row, loaded.title_field());
            self.write_page(&loaded, row, &title, Body::Replace(edited.to_owned()))
                .await?;
            return Ok(true);
        };
        let mut clean = true;
        self.change::<TextContent>(page, |current| {
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
        let mut additions = Vec::new();
        for (index, name) in header.iter().enumerate() {
            let Some(field) = loaded.schema.field_named(name) else {
                continue;
            };
            if field.field_type != DatabaseFieldType::Enum {
                continue;
            }
            let mut wanted: Vec<String> = Vec::new();
            for row in &rows {
                let value = row.get(index).map(|cell| cell.trim()).unwrap_or_default();
                let known = field
                    .enum_options
                    .iter()
                    .any(|option| option.name.eq_ignore_ascii_case(value));
                if !value.is_empty()
                    && !known
                    && !wanted.iter().any(|held| held.eq_ignore_ascii_case(value))
                {
                    wanted.push(value.to_owned());
                }
            }
            for option in wanted {
                imported.options.push(format!("{}: {option}", field.name));
                additions.push(DatabaseSchema::add_enum_option(field.id, option).1);
            }
        }
        if !additions.is_empty() {
            self.change::<DatabaseSchemaContent>(schema, |_| Ok(additions.clone()))
                .await?;
            loaded = self.load(database).await?;
        }
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
                    Ok(field) if field.id == title_field => Column::Title,
                    Ok(field) => Column::Field(field),
                    Err(_) => {
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
            let mut title = String::new();
            let mut page_text = String::new();
            for (column, cell) in columns.iter().zip(row) {
                match column {
                    Column::Title => title = cell.trim().to_owned(),
                    Column::Page => page_text = cell.clone(),
                    Column::Ignored => {}
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
        Ok(imported)
    }

    async fn write_page(
        &self,
        loaded: &Loaded,
        row: ObjectId,
        title: &str,
        body: Body,
    ) -> Result<(), String> {
        let existing = self
            .load(loaded.id)
            .await?
            .database
            .rows
            .get(row)
            .and_then(|row| row.page());
        let page = match existing {
            Some(page) => page,
            None => {
                let page = Uuid::new_v4();
                self.peer
                    .create_block(
                        page,
                        TextContent::CONTENT_TYPE,
                        BlockParent::Block(loaded.id),
                        &BlockMetadata::named(title),
                    )
                    .await
                    .map_err(explain)?;
                self.peer
                    .save(page, &TextBlock::new("").content(), None)
                    .await
                    .map_err(explain)?;
                self.change::<DatabaseContent>(loaded.id, |_| {
                    Ok(vec![Database::set_page(row, Some(page))])
                })
                .await?;
                page
            }
        };
        self.change::<TextContent>(page, |current| {
            let edit = match &body {
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

    fn value(
        &self,
        field: &DatabaseField,
        text: &str,
        labels: &ValueLabels,
    ) -> Result<Option<DatabaseValue>, String> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        let wanted = text.trim();
        let value = match field.field_type {
            DatabaseFieldType::Enum => field
                .enum_options
                .iter()
                .find(|option| option.name.eq_ignore_ascii_case(wanted))
                .map(|option| DatabaseValue::Enum(option.id)),
            DatabaseFieldType::Relation => {
                let database = field
                    .relation_options
                    .database
                    .ok_or_else(|| format!("{} does not link to a database yet", field.name))?;
                labels
                    .rows_of(database)
                    .into_iter()
                    .find(|(link, title)| {
                        title.eq_ignore_ascii_case(wanted)
                            || be_block::database::short_id("", link.row)
                                .eq_ignore_ascii_case(wanted.rsplit('-').next().unwrap_or(wanted))
                    })
                    .map(|(link, _)| DatabaseValue::Relation(link))
            }
            DatabaseFieldType::Block => Uuid::parse_str(wanted).ok().map(DatabaseValue::Block),
            _ => parse_cell_value(text, field),
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

fn replace_text(current: &TextContent, text: &str) -> Option<be_block::Edit> {
    let now = TextBlock::text(current);
    if now == text {
        return None;
    }
    TextBlock::replace(current, CLIENT, 0..now.len(), text.as_bytes())
}
