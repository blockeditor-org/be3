//! The account's own block (guides/blocks/keys.md, "Contacts"): what the account keeps for
//! itself (its contacts' records, its links' codes, what it pins per organization), owner-only, in the personal organization, at an id derived from the account. Each
//! record is the bytes its keeper encodes (`be_block_client::contacts::OwnContact`), sealed only
//! by the block's own keys, which no one but the account opens.

use be_block_model::{Bytes, Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

use be_block_content::{ChildChange, Root};

const ACCOUNT_BLOCK_MASK: u128 = 0x6163_636f_756e_742d_6f77_6e2d_626c_6b31;

/// The account's own block in `organization`, its personal organization: made at the top,
/// owner-only, by the account alone.
pub const fn account_block(organization: Uuid, account: Uuid) -> Uuid {
    Uuid::from_u128(organization.as_u128() ^ account.as_u128() ^ ACCOUNT_BLOCK_MASK)
}

/// The account whose own block `block` would be.
pub fn account_block_of(organization: Uuid, block: Uuid) -> Uuid {
    Uuid::from_u128(organization.as_u128() ^ block.as_u128() ^ ACCOUNT_BLOCK_MASK)
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct AccountBlock {
    /// The account's record of each contact, by the contact's account.
    pub contacts: Map<Uuid, Bytes>,
    /// The code of each link the account made and what it does, by link, until it is finished.
    pub links: Map<Uuid, Bytes>,
    /// What the account pins for each company organization it is in: its access log's root and
    /// escrow key, by organization.
    pub pins: Map<Uuid, Bytes>,
}

impl AccountBlock {
    pub fn contact(&self, account: Uuid) -> Option<&[u8]> {
        self.contacts.get(&account).map(|bytes| &bytes[..])
    }

    pub fn set_contact(account: Uuid, record: Option<Vec<u8>>) -> Edit {
        Self::CONTACTS
            .put(ObjectId::ROOT, &account, record.map(Bytes::from).as_ref())
            .into()
    }

    pub fn link(&self, link: Uuid) -> Option<&[u8]> {
        self.links.get(&link).map(|bytes| &bytes[..])
    }

    pub fn set_link(link: Uuid, kept: Option<Vec<u8>>) -> Edit {
        Self::LINKS
            .put(ObjectId::ROOT, &link, kept.map(Bytes::from).as_ref())
            .into()
    }

    pub fn pin(&self, organization: Uuid) -> Option<&[u8]> {
        self.pins.get(&organization).map(|bytes| &bytes[..])
    }

    pub fn set_pin(organization: Uuid, pinned: Option<Vec<u8>>) -> Edit {
        Self::PINS
            .put(ObjectId::ROOT, &organization, pinned.map(Bytes::from).as_ref())
            .into()
    }
}

impl Root for AccountBlock {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6163_636f_756e_742d_626c_6f63_6b30_31);

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
    }

    fn child_edit(&self, _: ChildChange) -> Option<Edit> {
        None
    }
}

pub type AccountBlockContent = Document<AccountBlock>;
