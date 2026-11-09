use be_block_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::Root;

/// A view of boat.dev sandboxes: only which account it shows and how it filters them. The
/// sandboxes themselves are never content: each viewer fetches them with their own key, so
/// boat's access decides what they see and sharing the block shares no machine's name or
/// address (decisions/live_data_from_an_outside_service_is_not_block_content.md).
#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Boat {
    /// The boat.dev organization, by id or name, empty for the key's own account.
    pub organization: String,
    /// Shows only the sandboxes whose name starts with this, when not empty.
    pub name_prefix: String,
    /// Shows only the sandboxes with this `key=value` label, when not empty.
    pub label: String,
    /// The terminal blocks connected to its sandboxes, keyed by block id: references, so a
    /// viewer who can open one sees it in the sandbox's row. A terminal of someone who may not
    /// edit the block is in their Unfiled alone.
    pub terminals: Map<String, BoatTerminal>,
}

/// A terminal block whose host runs in a sandbox.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct BoatTerminal {
    /// The sandbox's id, `bx_...`.
    pub sandbox: String,
}

impl Boat {
    pub fn set_organization(organization: impl Into<String>) -> Edit {
        Self::ORGANIZATION
            .set(ObjectId::ROOT, &organization.into().trim().to_owned())
            .into()
    }

    pub fn set_name_prefix(prefix: impl Into<String>) -> Edit {
        Self::NAME_PREFIX.set(ObjectId::ROOT, &prefix.into()).into()
    }

    pub fn set_label(label: impl Into<String>) -> Edit {
        Self::LABEL
            .set(ObjectId::ROOT, &label.into().trim().to_owned())
            .into()
    }

    pub fn add_terminal(block: Uuid, sandbox: impl Into<String>) -> Edit {
        let terminal = BoatTerminal {
            sandbox: sandbox.into(),
        };
        Self::TERMINALS
            .put(ObjectId::ROOT, &block.to_string(), Some(&terminal))
            .into()
    }

    pub fn remove_terminal(block: Uuid) -> Edit {
        Self::TERMINALS
            .put(ObjectId::ROOT, &block.to_string(), None)
            .into()
    }

    /// The terminal blocks connected to `sandbox`, oldest first (block ids are UUIDv7).
    pub fn terminals_of(&self, sandbox: &str) -> Vec<Uuid> {
        self.terminals
            .iter()
            .filter(|(_, terminal)| terminal.sandbox == sandbox)
            .filter_map(|(block, _)| Uuid::parse_str(block).ok())
            .collect()
    }

    /// The `key=value` a label filter asks for, if it is one.
    pub fn label_filter(label: &str) -> Option<(&str, &str)> {
        let (key, value) = label.split_once('=')?;
        let key = key.trim();
        (!key.is_empty()).then_some((key, value.trim()))
    }
}

impl Root for Boat {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x626f_6174_2d62_6c6f_636b_0000_0000_0001);

    fn name(&self) -> Option<String> {
        (!self.organization.is_empty()).then(|| format!("boat.dev: {}", self.organization))
    }

    fn references(&self) -> Vec<Uuid> {
        self.terminals
            .keys()
            .filter_map(|block| Uuid::parse_str(block).ok())
            .collect()
    }
}

pub type BoatContent = Document<Boat>;
