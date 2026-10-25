use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use be_block::{BlockContent, BlockMetadata, LiveEdit, ObjectId};
use be_client::{Peer, Saved};
use be_commit::CommitId;
use be_graph::BlockParent;
use be_protocol::BlockSummary;
use be_store::ObjectStore;
use be_types_host::Modules;
use block_type_api::{Change as Edit, ChildChange, Description};
use block_ui::database::{ValueLabels, database_value_text, parse_cell_value};
use database_types::database::{
    Database, DatabaseContent, DatabaseValue, Inverse, RowRef, inverses, short_id,
};
use database_types::database_schema::{
    ChoiceColor, ChoiceIcon, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
    DatabaseRelationOptions, DatabaseSchema, DatabaseSchemaContent,
};
use database_types::database_view::{DatabaseView, DatabaseViewContent, DatabaseViewKind};
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard};
use uuid::Uuid;

use crate::blocks::{Entry, Found, Index, find_in, is_top, short_of};
use crate::session::explain;
use text_block_types::TextContent;

mod prefixes;

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

enum Column {
    Title,
    Page,
    Field(Box<DatabaseField>),
    Ignored,
}

#[derive(Default)]
struct Labels {
    shown: ValueLabels,
    aliases: Vec<(RowRef, String)>,
}

impl Labels {
    fn add_aliases(&mut self, database: Uuid, rows: &Database, schema: &DatabaseSchema) {
        let texts: Vec<Uuid> = schema
            .fields()
            .into_iter()
            .filter(|field| field.field_type == DatabaseFieldType::String)
            .map(|field| field.id)
            .collect();
        for row in rows.rows.iter() {
            let link = RowRef {
                database,
                row: row.id.as_uuid(),
            };
            for field in &texts {
                if let Some(DatabaseValue::String(text)) = row.value(*field)
                    && !text.trim().is_empty()
                {
                    self.aliases.push((link, text.trim().to_owned()));
                }
            }
        }
    }
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

    fn row(&self, row: ObjectId) -> Option<database_types::database::DatabaseRow> {
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

pub type Sources = Box<dyn Fn() -> Vec<be_types_host::Source>>;

pub struct Tracker<S: ObjectStore> {
    peer: Arc<Peer<S>>,
    index: Mutex<Index>,
    sources: Sources,
    types: Mutex<Option<Modules>>,
}

impl<S: ObjectStore> Tracker<S> {
    pub fn new(peer: Peer<S>, sources: Sources) -> Self {
        Self {
            peer: Arc::new(peer),
            index: Mutex::new(Index::default()),
            sources,
            types: Mutex::new(None),
        }
    }

    pub async fn types(&self) -> MappedMutexGuard<'_, Modules> {
        let mut held = self.types.lock().await;
        if held.is_none() {
            *held = Some(be_types_host::started((self.sources)()).await);
        }
        MutexGuard::map(held, |held| {
            held.as_mut().expect("the type modules started")
        })
    }

    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    async fn read_bytes(&self, block: Uuid) -> Result<(Option<CommitId>, Option<Vec<u8>>), String> {
        let read = self
            .peer
            .read_contents(&[(block, self.peer.local_head(block))])
            .await
            .map_err(explain)?;
        let Some((summary, bytes)) = read.into_iter().next() else {
            return Ok((None, None));
        };
        if let Some(head) = summary.head {
            self.peer.remember(block, head).map_err(explain)?;
        }
        Ok((summary.head, bytes))
    }

    async fn publish(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        expected: Option<CommitId>,
    ) -> Result<bool, String> {
        let description = self
            .types()
            .await
            .describe(content_type, bytes, self.peer.organization())
            .await?;
        match self
            .peer
            .publish_encoded(
                block,
                content_type,
                bytes,
                description.references,
                description.derived,
                expected,
                None,
            )
            .await
            .map_err(explain)?
        {
            Saved::Rejected { .. } => Ok(false),
            Saved::Published(_) | Saved::Unchanged(_) => Ok(true),
        }
    }

    async fn save<C: BlockContent>(&self, block: Uuid, content: &C) -> Result<(), String> {
        match self
            .publish(block, C::CONTENT_TYPE, &content.encode(), None)
            .await?
        {
            true => Ok(()),
            false => Err(format!(
                "block {block} was saved by someone else first; try again"
            )),
        }
    }

    async fn edit(
        &self,
        block: Uuid,
        content_type: Uuid,
        mut make: impl AsyncFnMut(Option<Vec<u8>>) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<Option<Vec<u8>>, String> {
        for _ in 0..ATTEMPTS {
            let (head, current) = self.read_bytes(block).await?;
            let Some(changed) = make(current.clone()).await? else {
                return Ok(current);
            };
            if Some(&changed) == current.as_ref() {
                return Ok(current);
            }
            if self.publish(block, content_type, &changed, head).await? {
                self.invalidate().await;
                return Ok(Some(changed));
            }
        }
        Err(format!(
            "block {block} kept changing while this was being saved; try again"
        ))
    }

    pub fn peer(&self) -> &Peer<S> {
        &self.peer
    }

    pub fn shared_peer(&self) -> Arc<Peer<S>> {
        Arc::clone(&self.peer)
    }

    async fn entries(&self, summaries: Vec<BlockSummary>) -> Result<Vec<Entry>, String> {
        self.peer.learn(&summaries).await.map_err(explain)?;
        Ok(summaries
            .into_iter()
            .filter_map(|summary| {
                let parent = summary.parent?;
                let metadata = self.peer.metadata(&summary).unwrap_or_default();
                Some(Entry {
                    id: summary.id,
                    short: short_of(summary.id),
                    block_type: summary.content_type,
                    parent,
                    name: metadata.name(),
                    prefix: metadata.get(&be_block::ROW_PREFIX),
                    references: summary.references,
                    access: summary.access,
                    head: summary.head,
                    deleted: false,
                })
            })
            .collect())
    }

    pub async fn top(&self) -> Result<Vec<Entry>, String> {
        if let Some(listed) = self.index.lock().await.listed(None) {
            return Ok(listed);
        }
        let (roots, shared) = futures_util::future::try_join(
            self.peer.children(BlockParent::Root),
            self.peer.shared(),
        )
        .await
        .map_err(explain)?;
        let mut summaries = roots;
        summaries.extend(shared);
        let entries = self.entries(summaries).await?;
        let entries = entries.into_iter().filter(is_top).collect();
        let mut index = self.index.lock().await;
        index.list(None, entries);
        Ok(index.listed(None).unwrap_or_default())
    }

    pub async fn children(&self, parent: Uuid) -> Result<Vec<Entry>, String> {
        if let Some(listed) = self.index.lock().await.listed(Some(parent)) {
            return Ok(listed);
        }
        let summaries = self
            .peer
            .children(BlockParent::Block(parent))
            .await
            .map_err(explain)?;
        let entries = self.entries(summaries).await?;
        let mut index = self.index.lock().await;
        index.list(Some(parent), entries);
        Ok(index.listed(Some(parent)).unwrap_or_default())
    }

    pub async fn entry(&self, id: Uuid) -> Result<Entry, String> {
        if let Some(entry) = self.index.lock().await.get(id) {
            return Ok(entry.clone());
        }
        let summary = self.peer.summary(id).await.map_err(explain)?;
        let mut deleted = !summary.held;
        let mut entry = self
            .entries(vec![summary])
            .await?
            .pop()
            .ok_or_else(|| format!("this account cannot see {id}"))?;
        let mut parent = entry.parent;
        while let BlockParent::Block(above) = parent
            && !deleted
        {
            match self.peer.summary(above).await {
                Ok(summary) if summary.access.can_view() => {
                    deleted = !summary.held;
                    parent = summary.parent.unwrap_or(BlockParent::Root);
                }
                _ => break,
            }
        }
        entry.deleted = deleted;
        self.index.lock().await.insert(entry.clone());
        Ok(entry)
    }

    pub async fn is_hidden(&self, block: Uuid) -> bool {
        !self
            .entry(block)
            .await
            .is_ok_and(|entry| entry.access.can_view())
    }

    pub async fn find(&self, text: &str, accept: impl Fn(&Entry) -> bool) -> Result<Entry, String> {
        let wanted = text.trim();
        if let Ok(id) = Uuid::parse_str(wanted.rsplit('/').next().unwrap_or(wanted)) {
            return self.entry(id).await;
        }
        let mut level = self.top().await?;
        while !level.is_empty() {
            match find_in(&level, wanted, &accept) {
                Found::One(entry)
                    if entry.block_type == DatabaseViewContent::CONTENT_TYPE
                        && entry.label().eq_ignore_ascii_case(wanted) =>
                {
                    let inside = self.children(entry.id).await?.into_iter().find(|child| {
                        child.block_type == DatabaseContent::CONTENT_TYPE
                            && child.label() == entry.label()
                    });
                    return Ok(inside.unwrap_or_else(|| entry.clone()));
                }
                Found::One(entry) => return Ok(entry.clone()),
                Found::Ambiguous(error) => return Err(error),
                Found::None => {}
            }
            let below = futures_util::future::try_join_all(
                level
                    .iter()
                    .filter(|entry| entry.access.can_view())
                    .map(|entry| self.children(entry.id)),
            )
            .await?;
            level = below.into_iter().flatten().collect();
        }
        Err(format!(
            "nothing is called {wanted}; `be ls` lists the blocks at the top"
        ))
    }

    pub async fn read<C: BlockContent>(&self, block: Uuid) -> Result<Option<C>, String> {
        let head = self.index.lock().await.get(block).map(|entry| entry.head);
        match head {
            Some(Some(head)) => Ok(Some(
                self.peer.open_commit::<C>(head).await.map_err(explain)?,
            )),
            Some(None) => Ok(None),
            None => self.peer.open::<C>(block).await.map_err(explain),
        }
    }

    pub async fn outermost(&self, entry: &Entry) -> Result<Entry, String> {
        if entry.block_type != DatabaseContent::CONTENT_TYPE {
            return Ok(entry.clone());
        }
        let shell = match entry.parent.block() {
            Some(parent) => self.entry(parent).await.ok(),
            None => None,
        };
        Ok(shell
            .filter(|parent| parent.block_type == DatabaseViewContent::CONTENT_TYPE)
            .unwrap_or_else(|| entry.clone()))
    }

    pub async fn place(&self, parent: BlockParent, child: Uuid) -> Result<(), String> {
        self.relist(parent, ChildChange::Add(child)).await
    }

    async fn relist(&self, parent: BlockParent, change: ChildChange) -> Result<(), String> {
        let BlockParent::Block(parent) = parent else {
            return Ok(());
        };
        let refused = || format!("{parent} cannot list blocks; do this in the app");
        let content_type = self
            .peer
            .summary(parent)
            .await
            .map_err(explain)?
            .content_type;
        if be_types_host::plain_of(content_type) != be_types_host::Plain::Directory {
            return Err(refused());
        }
        let mut listed = true;
        self.edit(parent, content_type, async |current| {
            let changed = self
                .types()
                .await
                .apply(content_type, current, CLIENT, Edit::Child(change))
                .await?;
            listed = changed.is_some();
            Ok(changed)
        })
        .await?;
        match listed {
            true => Ok(()),
            false => Err(refused()),
        }
    }

    pub async fn move_block(&self, entry: &Entry, to: &Entry) -> Result<String, String> {
        let moved = &self.outermost(entry).await?;
        let from = self.peer.summary(moved.id).await.map_err(explain)?.parent;
        let into = BlockParent::Block(to.id);
        if from == Some(into) {
            return Ok(format!("{} is already in {}", moved.label(), to.label()));
        }
        if self.lies_below(Some(moved.id), to.id).await || moved.id == to.id {
            return Err(format!("{} cannot move into itself", moved.label()));
        }
        self.place(into, moved.id).await?;
        self.peer
            .set_parent(moved.id, into)
            .await
            .map_err(explain)?;
        if let Some(from @ BlockParent::Block(_)) = from {
            self.relist(from, ChildChange::Delete(moved.id)).await?;
        }
        self.invalidate().await;
        Ok(format!(
            "moved {} ({}) into {} ({})",
            moved.label(),
            moved.short,
            to.label(),
            to.short
        ))
    }

    pub async fn delete_block(&self, entry: &Entry) -> Result<String, String> {
        let deleted = &self.outermost(entry).await?;
        match self.peer.summary(deleted.id).await.map_err(explain)?.parent {
            Some(BlockParent::Block(parent)) => {
                self.relist(BlockParent::Block(parent), ChildChange::Delete(deleted.id))
                    .await?;
            }
            _ => self.peer.delete(deleted.id).await.map_err(explain)?,
        }
        self.invalidate().await;
        Ok(format!(
            "deleted {} ({}) and everything inside it; the app lists it under Recently Deleted",
            deleted.label(),
            deleted.short
        ))
    }

    pub async fn invalidate(&self) {
        *self.index.lock().await = Index::default();
    }

    pub async fn resolve(&self, text: &str) -> Result<Entry, String> {
        self.find(text, |_| true).await
    }

    pub async fn database_of(&self, entry: &Entry) -> Result<Uuid, String> {
        if entry.block_type == DatabaseContent::CONTENT_TYPE {
            return Ok(entry.id);
        }
        if entry.block_type == DatabaseViewContent::CONTENT_TYPE {
            let view = self
                .read::<DatabaseViewContent>(entry.id)
                .await?
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
        let entry = self
            .find(wanted, |entry| {
                entry.block_type == DatabaseContent::CONTENT_TYPE
                    || entry.block_type == DatabaseViewContent::CONTENT_TYPE
            })
            .await?;
        self.database_of(&entry).await
    }

    async fn find_row(&self, database: Uuid, code: &str) -> Result<ObjectId, String> {
        let loaded = self.load(database).await?;
        match loaded.database.rows_with_short_id(code).as_slice() {
            [row] => Ok(*row),
            [] => Err(format!("{} has no row {code}", loaded.name)),
            _ => Err(format!(
                "more than one row of {} has the id {code}; use `be {} rows` to find the one you mean by its title",
                loaded.name, loaded.name
            )),
        }
    }

    async fn load_plain(&self, id: Uuid) -> Result<(Database, DatabaseSchema), String> {
        let database = self
            .read::<DatabaseContent>(id)
            .await?
            .map(|content| content.root())
            .unwrap_or_default();
        let schema = match database.schema {
            Some(schema) => self
                .read::<DatabaseSchemaContent>(schema)
                .await?
                .map(|content| content.root())
                .unwrap_or_default(),
            None => DatabaseSchema::default(),
        };
        Ok((database, schema))
    }

    async fn load(&self, id: Uuid) -> Result<Loaded, String> {
        let name = self
            .entry(id)
            .await
            .map_or_else(|_| "Database".to_owned(), |entry| entry.label());
        let (database, schema) = self.load_plain(id).await?;
        let mut sources: Vec<Uuid> = self
            .peer
            .backrefs(id)
            .await
            .map_err(explain)?
            .into_iter()
            .filter(|summary| summary.content_type == DatabaseSchemaContent::CONTENT_TYPE)
            .filter_map(|summary| summary.parent.and_then(BlockParent::block))
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

    async fn labels(&self, loaded: &Loaded) -> Result<Labels, String> {
        let mut labels = Labels::default();
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
            labels.add_aliases(target, &database, &schema);
            let title = schema.title_field();
            for row in database.rows.iter() {
                labels.shown.rows.insert(
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

    fn view(&self, loaded: &Loaded, row: ObjectId, labels: &Labels) -> RowView {
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
                    let shown = labels.shown.row(link);
                    match shown.split_once(' ') {
                        Some((id, title)) => (id.to_owned(), title.to_owned()),
                        None => (shown, String::new()),
                    }
                })
                .collect();
                Some(FieldView {
                    name: field.name.clone(),
                    text: database_value_text(&value, &field, &labels.shown),
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

    pub async fn list(&self, database: Uuid, filters: &[Filter]) -> Result<Vec<RowView>, String> {
        let loaded = self.load(database).await?;
        let labels = self.labels(&loaded).await?;
        let filters: Vec<Filter> = filters
            .iter()
            .map(|filter| {
                let field = match loaded.field(&filter.field) {
                    Ok(Writable::Own(field)) => field,
                    Ok(Writable::Inverse(inverse)) => inverse.field(),
                    Err(error) => return Err(error),
                };
                if field.field_type != DatabaseFieldType::Relation {
                    return Ok(filter.clone());
                }
                let resolve = |value: &String| {
                    self.link(&field, value, &labels)
                        .map(|link| {
                            let shown = labels.shown.row(&link);
                            shown.split(' ').next().unwrap_or_default().to_owned()
                        })
                        .unwrap_or_else(|_| value.clone())
                };
                let condition = match &filter.condition {
                    Condition::Is(value) => Condition::Is(resolve(value)),
                    Condition::IsNot(value) => Condition::IsNot(resolve(value)),
                    other => other.clone(),
                };
                Ok(Filter {
                    field: filter.field.clone(),
                    condition,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(loaded
            .database
            .rows
            .iter()
            .map(|row| self.view(&loaded, row.id, &labels))
            .filter(|row| filters.iter().all(|filter| filter.matches(row)))
            .collect())
    }

    pub async fn show(&self, database: Uuid, code: &str) -> Result<RowView, String> {
        let row = self.find_row(database, code).await?;
        self.show_row(database, row).await
    }

    async fn show_row(&self, database: Uuid, row: ObjectId) -> Result<RowView, String> {
        self.show_written(database, row, None, None).await
    }

    async fn show_written(
        &self,
        database: Uuid,
        row: ObjectId,
        written: Option<Database>,
        body: Option<String>,
    ) -> Result<RowView, String> {
        let mut loaded = self.load(database).await?;
        if let Some(written) = written {
            loaded.database = written;
        }
        let labels = self.labels(&loaded).await?;
        let mut view = self.view(&loaded, row, &labels);
        view.body = match (body, view.page) {
            (Some(body), _) => Some(body),
            (None, Some(page)) => Some(self.text(page).await?),
            (None, None) => None,
        };
        Ok(view)
    }

    pub async fn schema(&self, database: Uuid) -> Result<Vec<String>, String> {
        let loaded = self.load(database).await?;
        let title = loaded.title_field();
        let linked = loaded
            .fields()
            .into_iter()
            .filter_map(|field| field.relation_options.database)
            .chain(loaded.inverses.iter().map(|inverse| inverse.source));
        let mut names = HashMap::new();
        for id in linked {
            let name = self.entry(id).await.map_or_else(
                |_| id.to_string(),
                |entry| format!("{} ({})", entry.label(), entry.short),
            );
            names.insert(id, name);
        }
        let name_of = |id: Uuid| names.get(&id).cloned().unwrap_or_else(|| id.to_string());
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
                        .map(|option| option_text(&field, option))
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
        let (view, database, schema) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let named = BlockMetadata::named(name);
        self.peer
            .create_block(view, DatabaseViewContent::CONTENT_TYPE, parent, &named)
            .await
            .map_err(explain)?;
        let mut shown = DatabaseView::of(database);
        shown.kind = DatabaseViewKind::List;
        self.save(view, &DatabaseViewContent::new(&shown)).await?;
        self.place(parent, view).await?;
        let mut held = BlockMetadata::named(name);
        held.insert(&be_block::ROW_PREFIX, &prefix);
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
        self.save(schema, &columns).await?;
        let mut content = DatabaseContent::new(&Database::with_schema(schema));
        content.apply(&Database::set_prefix(prefix.clone()));
        self.save(database, &content).await?;
        self.register(database, &prefix).await?;
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

    pub async fn set_option(
        &self,
        database: Uuid,
        field: &str,
        option: &str,
        settings: &[Assignment],
        adding: bool,
    ) -> Result<(), String> {
        let loaded = self.load(database).await?;
        let schema = loaded
            .database
            .schema
            .ok_or_else(|| format!("{} has no schema", loaded.name))?;
        let field = loaded
            .schema
            .field_named(field)
            .filter(|field| field.field_type == DatabaseFieldType::Enum)
            .ok_or_else(|| format!("{} has no choice column called {field}", loaded.name))?;
        let named = |name: &str| {
            field
                .enum_options
                .iter()
                .find(|option| option.name.eq_ignore_ascii_case(name.trim()))
                .map(|option| option.id)
                .ok_or_else(|| {
                    format!(
                        "{} is not one of {}: {}",
                        name.trim(),
                        field.name,
                        option_names(&field)
                    )
                })
        };
        if option.trim().is_empty() {
            return Err(format!("name an option of {}", field.name));
        }
        let mut edits = Vec::new();
        let id = match (named(option), adding) {
            (Ok(id), false) => id,
            (Err(error), false) => return Err(error),
            (Ok(_), true) => {
                return Err(format!("{} already has {}", field.name, option.trim()));
            }
            (Err(_), true) => {
                let (id, add) = DatabaseSchema::add_enum_option(field.id, option.trim());
                edits.push(add);
                id
            }
        };
        for setting in settings {
            let value = setting.value.trim();
            if setting.change != Change::Set {
                return Err(format!("an option takes {}=VALUE", setting.field));
            }
            let edit = match setting.field.to_ascii_lowercase().as_str() {
                "color" | "colour" => DatabaseSchema::set_option_color(
                    id,
                    ChoiceColor::named(value).ok_or_else(|| {
                        format!(
                            "{value} is not a colour; use {}",
                            ChoiceColor::ALL.map(ChoiceColor::name).join(", ")
                        )
                    })?,
                ),
                "icon" => DatabaseSchema::set_option_icon(
                    id,
                    ChoiceIcon::named(value).ok_or_else(|| {
                        format!(
                            "{value} is not an icon; use {}",
                            ChoiceIcon::ALL.map(ChoiceIcon::name).join(", ")
                        )
                    })?,
                ),
                "closed" => DatabaseSchema::set_option_closed(id, yes(&setting.field, value)?),
                "default" => match yes(&setting.field, value)? {
                    true => DatabaseSchema::set_default_option(field.id, Some(id)),
                    false if field.default_option == Some(id) => {
                        DatabaseSchema::set_default_option(field.id, None)
                    }
                    false => continue,
                },
                "name" if value.is_empty() => return Err("an option needs a name".to_owned()),
                "name" => DatabaseSchema::rename_enum_option(id, value),
                "after" => match named(value)? {
                    after if after == id => continue,
                    after => DatabaseSchema::move_enum_option(field.id, id, Some(after)),
                },
                "first" => match yes(&setting.field, value)? {
                    true => DatabaseSchema::move_enum_option(field.id, id, None),
                    false => continue,
                },
                other => {
                    return Err(format!(
                        "an option takes color, icon, closed, default, name, after and first, not {other}"
                    ));
                }
            };
            edits.push(edit);
        }
        self.change::<DatabaseSchemaContent>(schema, |_| Ok(edits.clone()))
            .await?;
        self.invalidate().await;
        Ok(())
    }

    pub async fn closed_option(
        &self,
        database: Uuid,
        field: &str,
    ) -> Result<Option<String>, String> {
        let (_, schema) = self.load_plain(database).await?;
        Ok(schema
            .field_named(field)
            .and_then(|field| field.first_closed().map(|option| option.name.clone())))
    }

    pub async fn create(
        &self,
        database: Uuid,
        title: &str,
        assignments: &[Assignment],
        body: Body,
    ) -> Result<RowView, String> {
        if title.trim().is_empty() {
            return Err("a row needs a title".to_owned());
        }
        let loaded = self.load(database).await?;
        let title_field = loaded
            .title_field()
            .ok_or_else(|| format!("{} has no text column to hold a title", loaded.name))?;
        let named: Vec<Uuid> = assignments
            .iter()
            .filter_map(|assignment| match loaded.field(&assignment.field) {
                Ok(Writable::Own(field)) => Some(field.id),
                _ => None,
            })
            .collect();
        let defaults = loaded
            .schema
            .defaults()
            .into_iter()
            .filter(|(field, _)| !named.contains(field));
        let (row, add) = Database::add_row(
            std::iter::once((title_field, DatabaseValue::String(title.to_owned()))).chain(defaults),
        );
        self.change::<DatabaseContent>(database, |_| Ok(vec![add.clone()]))
            .await?;
        self.apply(database, row, assignments, body).await
    }

    pub async fn update(
        &self,
        database: Uuid,
        code: &str,
        assignments: &[Assignment],
        body: Body,
    ) -> Result<RowView, String> {
        let row = self.find_row(database, code).await?;
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
        let mut written = None;
        if !own.is_empty() {
            let content = self
                .change::<DatabaseContent>(database, |current| {
                    if current.root().rows.get(row).is_none() {
                        return Err(format!("{code} was deleted"));
                    }
                    Ok(own.clone())
                })
                .await?;
            written = Some(content.root());
        }
        for (source, edit) in elsewhere {
            self.change::<DatabaseContent>(source, |_| Ok(vec![edit.clone()]))
                .await?;
        }
        let page = loaded.database.rows.get(row).and_then(|row| row.page());
        if let (Some(title), Some(page)) = (&retitled, page) {
            self.peer
                .define(page, |metadata| metadata.insert(&be_block::NAME, title))
                .await
                .map_err(explain)?;
        }
        let mut text = None;
        if body != Body::Keep {
            let title =
                retitled.unwrap_or_else(|| loaded.database.title(row, loaded.title_field()));
            text = Some(self.write_page(database, row, &title, body).await?);
        }
        self.show_written(database, row, written, text).await
    }

    fn assigned(
        &self,
        loaded: &Loaded,
        row: ObjectId,
        current: &database_types::database::DatabaseRow,
        field: &DatabaseField,
        assignment: &Assignment,
        labels: &Labels,
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
        current: &database_types::database::DatabaseRow,
        field: &DatabaseField,
        assignment: &Assignment,
        labels: &Labels,
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

    fn link(&self, field: &DatabaseField, wanted: &str, labels: &Labels) -> Result<RowRef, String> {
        let database = field
            .relation_options
            .database
            .ok_or_else(|| format!("{} does not link to a database yet", field.name))?;
        let code = wanted.rsplit('-').next().unwrap_or(wanted);
        let rows = labels.shown.rows_of(database);
        let aliased: Vec<RowRef> = labels
            .aliases
            .iter()
            .filter(|(link, alias)| link.database == database && alias.eq_ignore_ascii_case(wanted))
            .map(|(link, _)| *link)
            .collect();
        let by_id: Vec<RowRef> = rows
            .iter()
            .filter(|(link, _)| {
                code.len() == 6 && short_id("", link.row).eq_ignore_ascii_case(code)
            })
            .map(|(link, _)| *link)
            .collect();
        let titled: Vec<RowRef> = rows
            .iter()
            .filter(|(_, label)| {
                label
                    .split_once(' ')
                    .is_some_and(|(_, title)| title.eq_ignore_ascii_case(wanted))
            })
            .map(|(link, _)| *link)
            .collect();
        for mut found in [aliased, by_id, titled] {
            found.sort();
            found.dedup();
            match found.as_slice() {
                [] => continue,
                [one] => return Ok(*one),
                _ => {
                    return Err(format!(
                        "more than one row matches {wanted} for {}; use its id",
                        field.name
                    ));
                }
            }
        }
        Err(format!("{} has no row {wanted} to link", field.name))
    }

    pub async fn delete(&self, database: Uuid, code: &str) -> Result<String, String> {
        let row = self.find_row(database, code).await?;
        let loaded = self.load(database).await?;
        let page = loaded.database.rows.get(row).and_then(|row| row.page());
        let title = loaded.database.title(row, loaded.title_field());
        self.change::<DatabaseContent>(database, |current| {
            let current = current.root();
            Ok(match current.rows.get(row) {
                Some(_) => vec![current.remove_row_and_links(database, row)],
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

    pub async fn row_pages(&self, database: Uuid) -> Result<Vec<Uuid>, String> {
        let (database, _) = self.load_plain(database).await?;
        Ok(database.rows.iter().filter_map(|row| row.page()).collect())
    }

    pub async fn page_of(
        &self,
        database: Uuid,
        code: &str,
        create: bool,
    ) -> Result<Option<Uuid>, String> {
        let row = self.find_row(database, code).await?;
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
        let (_, bytes) = self.read_bytes(block).await?;
        self.plain_text(TextContent::CONTENT_TYPE, bytes).await
    }

    pub async fn write_text(&self, block: Uuid, body: &Body) -> Result<String, String> {
        self.write_file(block, |now| match body {
            Body::Keep => None,
            Body::Replace(text) => Some(text.clone()),
            Body::Append(text) => Some(match now.is_empty() || now.ends_with('\n') {
                true => format!("{now}{text}"),
                false => format!("{now}\n{text}"),
            }),
        })
        .await
    }

    pub async fn rewrite_text(
        &self,
        block: Uuid,
        base: &str,
        edited: &str,
    ) -> Result<bool, String> {
        let mut clean = true;
        self.write_file(block, |now| {
            Some(match now == base {
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
            })
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
                    Ok(Writable::Own(field)) => Column::Field(Box::new(field)),
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
        let mut labels = self.labels(&loaded).await?;
        let defaults = loaded.schema.defaults();
        let mut edits = Vec::new();
        let mut later: Vec<(usize, ObjectId, DatabaseField, String)> = Vec::new();
        let mut pages = Vec::new();
        for (number, row) in rows.iter().enumerate() {
            let mut values = BTreeMap::new();
            let mut title = String::new();
            let mut page_text = String::new();
            for (column, cell) in columns.iter().zip(row) {
                match column {
                    Column::Title => title = cell.trim().to_owned(),
                    Column::Page => page_text = cell.clone(),
                    Column::Ignored => {}
                    Column::Field(field) if field.field_type == DatabaseFieldType::Relation => {
                        later.push((number, ObjectId::ROOT, (**field).clone(), cell.clone()));
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
            for (field, value) in &defaults {
                values.entry(*field).or_insert_with(|| value.clone());
            }
            for (_, text) in values.iter().filter_map(|(field, value)| match value {
                DatabaseValue::String(text) if !text.trim().is_empty() => Some((field, text)),
                _ => None,
            }) {
                labels.aliases.push((
                    RowRef {
                        database,
                        row: Uuid::nil(),
                    },
                    text.trim().to_owned(),
                ));
            }
            let (id, add) = Database::add_row(values);
            for alias in labels
                .aliases
                .iter_mut()
                .filter(|(link, _)| link.database == database && link.row.is_nil())
            {
                alias.0.row = id.as_uuid();
            }
            for pending in later.iter_mut().filter(|pending| pending.0 == number) {
                pending.1 = id;
            }
            edits.push(add);
            if !page_text.trim().is_empty() {
                let page = Uuid::now_v7();
                let (content, description) = self.text_page(&page_text).await?;
                pages.push((
                    be_client::transfer::Saved {
                        id: page,
                        block_type: TextContent::CONTENT_TYPE,
                        parent: None,
                        metadata: BlockMetadata::named(title).with_derived(description.derived),
                        references: self.peer.resolve(page, description.references),
                        content: Some(content),
                    },
                    BlockParent::Block(database),
                ));
                edits.push(Database::set_page(id, Some(page)));
                imported.pages += 1;
            }
            imported.rows += 1;
        }
        for (number, id, field, cell) in later {
            let items: Vec<&str> = match field.relation_options.multiple {
                true => cell.split(',').map(str::trim).collect(),
                false => vec![cell.trim()],
            };
            for item in items.into_iter().filter(|item| !item.is_empty()) {
                match self.link(&field, item, &labels) {
                    Ok(link) if field.relation_options.multiple => {
                        edits.push(Database::link(id, field.id, link));
                    }
                    Ok(link) => edits.push(Database::set_value(
                        id,
                        field.id,
                        Some(&DatabaseValue::Relation(link)),
                    )),
                    Err(error) => imported.problems.push(format!(
                        "row {}, {}: {error}",
                        number + 2,
                        field.name
                    )),
                }
            }
        }
        let mut failed = None;
        self.peer
            .import_blocks(&pages, |page, created| {
                if let Err(error) = created
                    && failed.is_none()
                {
                    failed = Some(format!(
                        "the page {page} was not created: {}",
                        explain(error)
                    ));
                }
            })
            .await;
        if let Some(error) = failed {
            return Err(error);
        }
        self.change::<DatabaseContent>(database, |_| Ok(edits.clone()))
            .await?;
        self.invalidate().await;
        Ok(imported)
    }

    async fn text_page(&self, text: &str) -> Result<(Vec<u8>, Description), String> {
        let mut types = self.types().await;
        let change = Edit::FromFile {
            name: String::new(),
            bytes: text.as_bytes().to_vec(),
        };
        let content = types
            .apply(TextContent::CONTENT_TYPE, None, CLIENT, change)
            .await?
            .ok_or("text has no plain-file form")?;
        let description = types
            .describe(
                TextContent::CONTENT_TYPE,
                &content,
                self.peer.organization(),
            )
            .await?;
        Ok((content, description))
    }

    async fn write_page(
        &self,
        database: Uuid,
        row: ObjectId,
        title: &str,
        body: Body,
    ) -> Result<String, String> {
        let (current, _) = self.load_plain(database).await?;
        let page = match current.rows.get(row).and_then(|row| row.page()) {
            Some(page) => page,
            None => {
                let page = Uuid::now_v7();
                self.peer
                    .create_block(
                        page,
                        TextContent::CONTENT_TYPE,
                        BlockParent::Block(database),
                        &BlockMetadata::named(title),
                    )
                    .await
                    .map_err(explain)?;
                let (blank, _) = self.text_page("").await?;
                if !self
                    .publish(page, TextContent::CONTENT_TYPE, &blank, None)
                    .await?
                {
                    return Err(format!("the page {page} was saved by someone else first"));
                }
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
        labels: &Labels,
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
                DatabaseFieldType::Enum => {
                    format!(
                        "{} is not one of {}: {}",
                        wanted,
                        field.name,
                        option_names(field)
                    )
                }
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
        C: BlockContent + LiveEdit + Clone + Default,
    {
        let changed = self
            .edit(block, C::CONTENT_TYPE, async |current| {
                let mut content = match current {
                    Some(bytes) => C::decode(&bytes).map_err(|error| error.to_string())?,
                    None => C::default(),
                };
                let edits = make(&content)?;
                if edits.is_empty() {
                    return Ok(None);
                }
                for edit in &edits {
                    content.apply(edit);
                }
                Ok(Some(content.encode()))
            })
            .await?;
        match changed {
            Some(bytes) => C::decode(&bytes).map_err(|error| error.to_string()),
            None => Ok(C::default()),
        }
    }

    async fn plain_text(
        &self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
    ) -> Result<String, String> {
        let Some(bytes) = bytes else {
            return Ok(String::new());
        };
        let file = self
            .types()
            .await
            .to_files(vec![(content_type, bytes)])
            .await
            .pop()
            .unwrap_or_else(|| Err("the type module did not answer".to_owned()))?
            .ok_or("this block has no plain-file form")?;
        Ok(String::from_utf8_lossy(&file.bytes).into_owned())
    }

    async fn write_file(
        &self,
        block: Uuid,
        mut wanted: impl FnMut(&str) -> Option<String>,
    ) -> Result<String, String> {
        let content_type = TextContent::CONTENT_TYPE;
        let written = self
            .edit(block, content_type, async |current| {
                let now = self.plain_text(content_type, current.clone()).await?;
                let Some(text) = wanted(&now) else {
                    return Ok(None);
                };
                let change = Edit::FromFile {
                    name: String::new(),
                    bytes: text.into_bytes(),
                };
                self.types()
                    .await
                    .apply(content_type, current, CLIENT, change)
                    .await
            })
            .await?;
        self.plain_text(content_type, written).await
    }
}

fn option_names(field: &DatabaseField) -> String {
    field
        .enum_options
        .iter()
        .map(|option| option.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn option_text(field: &DatabaseField, option: &DatabaseEnumOption) -> String {
    let mut traits = vec![option.color.name().to_owned()];
    if option.icon != ChoiceIcon::None {
        traits.push(format!("icon {}", option.icon.name()));
    }
    if option.closed {
        traits.push("closed".to_owned());
    }
    if field.default_value() == Some(option.id) {
        traits.push("default".to_owned());
    }
    format!("{} ({})", option.name, traits.join(", "))
}

fn yes(setting: &str, value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "yes" | "true" | "on" | "1" => Ok(true),
        "no" | "false" | "off" | "0" => Ok(false),
        _ => Err(format!("{setting} is yes or no, not {value}")),
    }
}

pub(crate) fn looks_like_row(text: &str) -> bool {
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
