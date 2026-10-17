use be_block::BlockContent;
use be_block::database::DatabaseContent;
use be_block::database_schema::DatabaseSchemaContent;
use be_block::database_view::DatabaseViewContent;
use be_block::text::TextContent;
use be_graph::{Access, BlockParent};
use uuid::Uuid;

const SHORTEST_ID: usize = 4;

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

#[derive(Clone, Debug, Default)]
pub struct Index {
    pub entries: Vec<Entry>,
}

impl Index {
    pub fn new(mut entries: Vec<Entry>) -> Self {
        let mut hex: Vec<(Uuid, String)> = entries
            .iter()
            .map(|entry| (entry.id, entry.id.simple().to_string()))
            .collect();
        hex.sort_by(|a, b| a.1.cmp(&b.1));
        for entry in &mut entries {
            let own = entry.id.simple().to_string();
            let position = hex.iter().position(|(id, _)| *id == entry.id).unwrap_or(0);
            let shared = |other: Option<&(Uuid, String)>| {
                other.map_or(0, |(_, text)| {
                    own.chars()
                        .zip(text.chars())
                        .take_while(|(a, b)| a == b)
                        .count()
                })
            };
            let before = position.checked_sub(1).and_then(|index| hex.get(index));
            let length = (shared(before).max(shared(hex.get(position + 1))) + 1).max(SHORTEST_ID);
            entry.short = own[..length.min(own.len())].to_owned();
        }
        entries.sort_by_key(|entry| entry.label().to_lowercase());
        Self { entries }
    }

    pub fn get(&self, id: Uuid) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn children(&self, parent: Option<Uuid>) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| match parent {
                Some(parent) => entry.parent == BlockParent::Block(parent),
                None => self.at_top(entry),
            })
            .filter(|entry| entry.access.can_view())
            .collect()
    }

    fn at_top(&self, entry: &Entry) -> bool {
        match entry.parent {
            BlockParent::Root => !is_settings(entry.block_type),
            BlockParent::Block(parent) => self.is_hidden(parent),
            BlockParent::Detached => false,
        }
    }

    pub fn is_hidden(&self, block: Uuid) -> bool {
        self.get(block)
            .is_none_or(|parent| !parent.access.can_view())
    }

    pub fn workspaces(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.block_type == be_block::WorkspaceContent::CONTENT_TYPE
                    && entry.parent == BlockParent::Root
                    && entry.access.can_view()
            })
            .collect()
    }

    pub fn default_parent(&self) -> Result<&Entry, String> {
        let workspaces: Vec<&Entry> = self
            .workspaces()
            .into_iter()
            .filter(|entry| entry.access.can_edit())
            .collect();
        let candidates = match workspaces.is_empty() {
            false => workspaces,
            true => self
                .entries
                .iter()
                .filter(|entry| {
                    entry.block_type == be_block::FolderContent::CONTENT_TYPE
                        && entry.access.can_edit()
                        && self.at_top(entry)
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

    pub fn find(&self, text: &str, accept: impl Fn(&Entry) -> bool) -> Result<&Entry, String> {
        let wanted = text.trim();
        if let Ok(id) = Uuid::parse_str(wanted.rsplit('/').next().unwrap_or(wanted))
            && let Some(entry) = self.get(id)
        {
            return Ok(entry);
        }
        let lower = wanted.to_ascii_lowercase().replace('-', "");
        let by_id: Vec<&Entry> = match lower.len() >= SHORTEST_ID
            && lower.chars().all(|character| character.is_ascii_hexdigit())
        {
            true => self
                .entries
                .iter()
                .filter(|entry| entry.id.simple().to_string().starts_with(&lower))
                .collect(),
            false => Vec::new(),
        };
        if let [one] = by_id.as_slice() {
            return Ok(one);
        }
        let mut named: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|entry| !entry.deleted && entry.label().eq_ignore_ascii_case(wanted))
            .collect();
        let shells: Vec<Uuid> = named
            .iter()
            .filter(|entry| entry.block_type == DatabaseContent::CONTENT_TYPE)
            .filter_map(|entry| entry.parent.block())
            .collect();
        named.retain(|entry| {
            !(entry.block_type == DatabaseViewContent::CONTENT_TYPE && shells.contains(&entry.id))
        });
        let accepted: Vec<&Entry> = named
            .iter()
            .copied()
            .filter(|entry| accept(entry))
            .collect();
        match (accepted.as_slice(), named.as_slice(), by_id.len()) {
            ([one], _, _) => Ok(one),
            ([], [one], _) => Ok(one),
            ([], [], 0) => Err(format!(
                "nothing is called {wanted}; `be ls` lists the blocks at the root"
            )),
            ([], [], _) => Err(format!(
                "{wanted} starts the ids of more than one block; use more of it"
            )),
            ([], several, _) | (several, _, _) => Err(format!(
                "more than one block is called {wanted}: {}; use an id",
                several
                    .iter()
                    .map(|entry| format!("{} ({})", entry.short, entry.kind()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

fn is_settings(block_type: Uuid) -> bool {
    block_type == be_block::SettingsContent::CONTENT_TYPE
        || block_type == be_block::SettingsProfileContent::CONTENT_TYPE
}

pub fn type_name(block_type: Uuid) -> &'static str {
    let known: [(Uuid, &str); 14] = [
        (DatabaseContent::CONTENT_TYPE, "database"),
        (DatabaseSchemaContent::CONTENT_TYPE, "schema"),
        (DatabaseViewContent::CONTENT_TYPE, "database view"),
        (TextContent::CONTENT_TYPE, "text"),
        (be_block::FolderContent::CONTENT_TYPE, "folder"),
        (be_block::WorkspaceContent::CONTENT_TYPE, "workspace"),
        (be_block::CanvasContent::CONTENT_TYPE, "canvas"),
        (be_block::ChecklistContent::CONTENT_TYPE, "checklist"),
        (be_block::CalendarContent::CONTENT_TYPE, "calendar"),
        (be_block::PresentationContent::CONTENT_TYPE, "presentation"),
        (be_block::ImageContent::CONTENT_TYPE, "image"),
        (be_block::SettingsContent::CONTENT_TYPE, "settings"),
        (
            be_block::SettingsProfileContent::CONTENT_TYPE,
            "settings profile",
        ),
        (be_block::PdfContent::CONTENT_TYPE, "pdf"),
    ];
    known
        .iter()
        .find(|(id, _)| *id == block_type)
        .map_or("block", |(_, name)| name)
}
