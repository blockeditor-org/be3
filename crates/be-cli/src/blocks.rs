use be_block::BlockContent;
use be_graph::{Access, BlockParent};
use uuid::Uuid;

const SHORTEST_ID: usize = 4;
const SHORT_ID: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: Uuid,
    pub short: String,
    pub block_type: Uuid,
    pub parent: BlockParent,
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub references: Vec<Uuid>,
    pub access: Access,
    pub head: Option<be_commit::CommitId>,
    pub deleted: bool,
}

impl Entry {
    pub fn label(&self) -> String {
        self.name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| type_name(self.block_type).to_owned())
    }

    pub fn kind(&self) -> &'static str {
        type_name(self.block_type)
    }
}

pub fn short_of(id: Uuid) -> String {
    let simple = id.simple().to_string();
    simple[simple.len() - SHORT_ID..].to_owned()
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    entries: std::collections::HashMap<Uuid, Entry>,
    listed: std::collections::HashMap<Option<Uuid>, Vec<Uuid>>,
}

impl Index {
    pub fn get(&self, id: Uuid) -> Option<&Entry> {
        self.entries.get(&id)
    }

    pub fn insert(&mut self, entry: Entry) {
        self.entries.insert(entry.id, entry);
    }

    pub fn listed(&self, parent: Option<Uuid>) -> Option<Vec<Entry>> {
        self.listed.get(&parent).map(|ids| {
            let mut entries: Vec<Entry> = ids
                .iter()
                .filter_map(|id| self.entries.get(id))
                .filter(|entry| entry.access.can_view())
                .cloned()
                .collect();
            entries.sort_by_key(|entry| entry.label().to_lowercase());
            entries
        })
    }

    pub fn list(&mut self, parent: Option<Uuid>, entries: Vec<Entry>) {
        let ids = entries.iter().map(|entry| entry.id).collect();
        for entry in entries {
            self.insert(entry);
        }
        self.listed.insert(parent, ids);
    }
}

pub fn workspaces(top: &[Entry]) -> Vec<&Entry> {
    top.iter()
        .filter(|entry| entry.parent == BlockParent::Root)
        .collect()
}

pub fn default_parent(top: &[Entry]) -> Result<&Entry, String> {
    let workspaces: Vec<&Entry> = workspaces(top)
        .into_iter()
        .filter(|entry| entry.access.can_edit())
        .collect();
    let candidates = match workspaces.is_empty() {
        false => workspaces,
        true => top
            .iter()
            .filter(|entry| {
                crate::plugins::adds_children(entry.block_type) && entry.access.can_edit()
            })
            .collect(),
    };
    match candidates.as_slice() {
        [one] => Ok(one),
        [] => Err(
            "this account cannot write in any workspace; ask someone to share one with it"
                .to_owned(),
        ),
        several => Err(format!(
            "this account can write in {} workspaces; name one with --in: {}",
            several.len(),
            several
                .iter()
                .map(|entry| format!("{} ({})", entry.label(), entry.short))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub enum Found<'a> {
    One(&'a Entry),
    None,
    Ambiguous(String),
}

pub fn find_in<'a>(level: &'a [Entry], text: &str, accept: impl Fn(&Entry) -> bool) -> Found<'a> {
    let wanted = text.trim();
    let lower = wanted.to_ascii_lowercase().replace('-', "");
    let by_id: Vec<&Entry> = match lower.len() >= SHORTEST_ID
        && lower.chars().all(|character| character.is_ascii_hexdigit())
    {
        true => level
            .iter()
            .filter(|entry| entry.id.simple().to_string().ends_with(&lower))
            .collect(),
        false => Vec::new(),
    };
    if let [one] = by_id.as_slice() {
        return Found::One(one);
    }
    let named: Vec<&Entry> = level
        .iter()
        .filter(|entry| !entry.deleted && entry.label().eq_ignore_ascii_case(wanted))
        .collect();
    let accepted: Vec<&Entry> = named
        .iter()
        .copied()
        .filter(|entry| accept(entry))
        .collect();
    match (accepted.as_slice(), named.as_slice(), by_id.len()) {
        ([one], _, _) => Found::One(one),
        ([], [one], _) => Found::One(one),
        ([], [], 0) => Found::None,
        ([], [], _) => Found::Ambiguous(format!(
            "{wanted} ends the ids of more than one block; use more of it"
        )),
        ([], several, _) | (several, _, _) => Found::Ambiguous(format!(
            "more than one block is called {wanted}: {}; use an id",
            several
                .iter()
                .map(|entry| format!("{} ({})", entry.short, entry.kind()))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub fn is_top(entry: &Entry) -> bool {
    !(entry.parent == BlockParent::Root && is_settings(entry.block_type))
}

fn is_settings(block_type: Uuid) -> bool {
    block_type == be_block::SettingsContent::CONTENT_TYPE
        || block_type == be_block::SettingsProfileContent::CONTENT_TYPE
}

pub fn type_name(block_type: Uuid) -> &'static str {
    let known: [(Uuid, &str); 3] = [
        (text_block_types::TextContent::CONTENT_TYPE, "text"),
        (be_block::SettingsContent::CONTENT_TYPE, "settings"),
        (
            be_block::SettingsProfileContent::CONTENT_TYPE,
            "settings profile",
        ),
    ];
    known
        .iter()
        .find(|(id, _)| *id == block_type)
        .map(|(_, name)| *name)
        .or_else(|| crate::plugins::name_of(block_type))
        .unwrap_or("block")
}
