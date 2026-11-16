use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use be_block_keys::{
    CarriedKey, EntryHash, EscrowSecret, Party, RecoveryPublic, RecoverySecret, SigningPublic,
};
use be_block_protocol::{EscrowKey, KeyKind, KeyRow, KeySet, WrappedBy, WrappedKey};
use be_block_store::{ContentKey, Keyring, StoreError};
use uuid::Uuid;

/// Its locks are taken in one order, `rows`, then `roots` and `trusted`, then
/// `escrow_secrets`, then `ring`'s own: `absorb` holds `rows` while it opens keys, so nothing
/// that holds a later lock takes an earlier one.
pub struct KeyBook {
    account: Uuid,
    organization: Uuid,
    identity: RecoverySecret,
    ring: Keyring,
    /// The escrow secrets this device holds, by escrow key id: X25519 secrets, never content
    /// keys, so kept out of `ring`, where an object whose header named one would open with it.
    escrow_secrets: Mutex<HashMap<Uuid, [u8; 32]>>,
    rows: Mutex<Rows>,
    contacts: Mutex<HashMap<Uuid, RecoveryPublic>>,
    trusted: Mutex<HashSet<Uuid>>,
    escrow: Mutex<Escrow>,
    server: Mutex<Option<(Uuid, RecoveryPublic)>>,
    roots: Mutex<Roots>,
    /// The hash of the organization's access log root, pinned from making it or an
    /// invitation (`crate::contacts::Pinned`).
    organization_root: Mutex<Option<EntryHash>>,
}

/// A key opened from what was sealed or wrapped, and whether it is an escrow's secret.
struct Opened {
    bytes: [u8; 32],
    escrow: bool,
}

/// Which block each content key is a key of, and the access log root each block's keys carry
/// (`be_block_keys::CarriedKey`): learned from keys made here and keys opened, never from the
/// server's rows alone.
#[derive(Default)]
struct Roots {
    blocks: HashMap<Uuid, Uuid>,
    pinned: HashMap<Uuid, EntryHash>,
    /// Blocks whose trusted keys carried different roots: one of them was forged, so neither
    /// is used.
    conflicting: HashSet<Uuid>,
}

/// What a device holds of a block's access log root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootPin {
    /// Pinned from making the block or from a trusted key of it.
    Pinned(EntryHash),
    /// Trusted keys of it carried different roots: one was forged.
    Conflicting,
    /// None yet.
    Unknown,
}

impl Roots {
    fn pin(&mut self, block: Uuid, root: EntryHash) {
        if self.conflicting.contains(&block) {
            return;
        }
        match self.pinned.get(&block) {
            Some(pinned) if *pinned != root => {
                self.pinned.remove(&block);
                self.conflicting.insert(block);
            }
            Some(_) => {}
            None => {
                self.pinned.insert(block, root);
            }
        }
    }
}

#[derive(Default)]
struct Rows {
    /// How many times keys were recorded: a read that failed for want of a key may succeed
    /// once it moves.
    revision: u64,
    keys: HashMap<Uuid, KeyRow>,
    wraps: HashMap<Uuid, Vec<WrappedKey>>,
    by_block: HashMap<Uuid, Vec<Uuid>>,
}

#[derive(Default)]
struct Escrow {
    chain: Vec<EscrowKey>,
    pinned: Option<EscrowKey>,
    verified: Option<EscrowKey>,
    /// The organization's administrators as its log checked names them, with their keys:
    /// those an epoch is taken from (`verify_escrow`). `None` until the log is read.
    administrators: Option<HashMap<Uuid, SigningPublic>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscrowWarning {
    Unverified,
    Substituted,
}

impl EscrowWarning {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unverified => {
                "This device cannot verify this organization's escrow key, so what it makes is \
                 not escrowed for the administrators until it can."
            }
            Self::Substituted => {
                "The server offers an escrow key this device cannot verify, so new keys stay \
                 escrowed under the last one it could."
            }
        }
    }
}

impl Rows {
    fn add_row(&mut self, row: KeyRow) {
        if self.keys.insert(row.id, row).is_none() {
            self.by_block.entry(row.block).or_default().push(row.id);
        }
    }

    fn add_wrap(&mut self, wrap: WrappedKey) {
        let held = self.wraps.entry(wrap.key).or_default();
        if let WrappedBy::Key(by) = wrap.wrapped_by
            && self
                .keys
                .get(&by)
                .is_none_or(|row| row.kind != KeyKind::Escrow)
        {
            held.retain(|other| match other.wrapped_by {
                WrappedBy::Key(other_by) => self
                    .keys
                    .get(&other_by)
                    .is_some_and(|row| row.kind == KeyKind::Escrow),
                WrappedBy::Account(_) => true,
            });
        }
        held.retain(|other| other.wrapped_by != wrap.wrapped_by);
        held.push(wrap);
    }
}

/// The latest escrow key this device trusts: the one it pinned, or a newer one it holds the
/// secret of, or, epoch by epoch, a newer one signed by its maker chained to the one before,
/// where the maker is one of `administrators` (the organization's log, checked to its head as
/// this device holds it). A demoted administrator, still holding the escrow keys of their time,
/// signs no epoch a device takes, nor does the server.
pub fn verify_escrow(
    organization: Uuid,
    pinned: Option<&EscrowKey>,
    chain: &[EscrowKey],
    administrators: Option<&HashMap<Uuid, SigningPublic>>,
    held: impl Fn(&EscrowKey) -> bool,
) -> Option<EscrowKey> {
    let mut sorted: Vec<&EscrowKey> = chain.iter().collect();
    sorted.sort_by_key(|escrow| escrow.epoch);
    let mut verified = pinned.cloned();
    for next in sorted {
        let newer = verified
            .as_ref()
            .is_none_or(|known| next.epoch > known.epoch);
        if held(next) && (newer || verified.as_ref().is_some_and(|known| known.id == next.id)) {
            verified = Some(next.clone());
            continue;
        }
        let maker = administrators.and_then(|administrators| administrators.get(&next.maker));
        if let Some(known) = &verified
            && newer
            && next.epoch == known.epoch + 1
            && let Some(maker) = maker
            && be_block_keys::verify_signature(
                &maker.0,
                &next.chained(organization, known.id),
                &next.signature,
            )
        {
            verified = Some(next.clone());
        }
    }
    verified
}

impl KeyBook {
    pub fn new(account: Uuid, organization: Uuid, identity: RecoverySecret, ring: Keyring) -> Self {
        Self {
            account,
            organization,
            identity,
            ring,
            escrow_secrets: Mutex::default(),
            rows: Mutex::new(Rows::default()),
            contacts: Mutex::new(HashMap::new()),
            trusted: Mutex::new(HashSet::new()),
            escrow: Mutex::new(Escrow::default()),
            server: Mutex::new(None),
            roots: Mutex::new(Roots::default()),
            organization_root: Mutex::new(None),
        }
    }

    pub fn set_server(&self, account: Uuid, public: RecoveryPublic) {
        *self.server.lock().unwrap() = Some((account, public));
    }

    pub fn server(&self) -> Option<(Uuid, RecoveryPublic)> {
        *self.server.lock().unwrap()
    }

    pub fn set_contacts(&self, verified: impl IntoIterator<Item = (Uuid, RecoveryPublic)>) {
        *self.contacts.lock().unwrap() = verified.into_iter().collect();
    }

    /// The public key this device seals to `account` with: its own, or a verified contact's.
    /// Never the server's, whatever account it names itself as: a key goes to the server only
    /// as the settings scope key (`recipient`, `Peer::share_with_server`).
    pub fn contact(&self, account: Uuid) -> Option<RecoveryPublic> {
        match account == self.account {
            true => Some(self.identity.public()),
            false => self.contacts.lock().unwrap().get(&account).copied(),
        }
    }

    /// The public key `key` is sealed to `account` with: `contact`'s, or the server's when
    /// `key` is a scope key of the organization's settings, which the server reads to enforce
    /// policies (a new epoch of it after a rotation reaches the server this way). Which block a
    /// key is of is what this device minted or opened it as, never the server's row.
    pub fn recipient(&self, account: Uuid, key: Uuid) -> Option<RecoveryPublic> {
        self.contact(account).or_else(|| {
            let settings = be_block::organization_settings(self.organization);
            let of_settings = self.roots.lock().unwrap().blocks.get(&key) == Some(&settings);
            let scope = self.row(key).is_some_and(|row| row.kind == KeyKind::Scope);
            self.server()
                .filter(|(server, _)| *server == account && of_settings && scope)
                .map(|(_, public)| public)
        })
    }

    /// The public key a key sealed by `account` opens with: this device's or a verified
    /// contact's (`contact`), else the server's, whose seals `accepts` limits to the settings
    /// key.
    pub fn sender(&self, account: Uuid) -> Option<RecoveryPublic> {
        self.contact(account).or_else(|| {
            self.server()
                .filter(|(server, _)| *server == account)
                .map(|(_, public)| public)
        })
    }

    /// Whether a key `sender` sealed to this account may open, for `block`: the server's seal
    /// only of the settings key, and only untrusted. This account and its verified contacts are
    /// themselves, whatever account the server names itself as.
    fn accepts(&self, sender: Uuid, block: Option<Uuid>, vouched: bool) -> bool {
        if self.contact(sender).is_some() {
            return true;
        }
        match self.server() {
            Some((server, _)) if server == sender => {
                !vouched && block == Some(be_block::organization_settings(self.organization))
            }
            _ => true,
        }
    }

    pub fn set_escrows(&self, chain: Vec<EscrowKey>) {
        {
            let mut rows = self.rows.lock().unwrap();
            for escrow in &chain {
                if !rows.keys.contains_key(&escrow.id) {
                    rows.add_row(KeyRow {
                        id: escrow.id,
                        block: self.organization,
                        kind: KeyKind::Escrow,
                        epoch: escrow.epoch,
                    });
                }
            }
        }
        self.escrow.lock().unwrap().chain = chain;
        self.settle_escrow();
    }

    /// The organization's administrators, with the signing keys its checked log names for them.
    pub fn set_administrators(&self, administrators: Option<HashMap<Uuid, SigningPublic>>) {
        self.escrow.lock().unwrap().administrators = administrators;
        self.settle_escrow();
    }

    /// What this account pinned for the organization: the escrow key, and the root of the
    /// organization's access log.
    pub fn set_pinned(&self, pinned: Option<crate::contacts::Pinned>) {
        let pinned = pinned.unwrap_or_default();
        *self.organization_root.lock().unwrap() = pinned.root;
        self.escrow.lock().unwrap().pinned = pinned.escrow;
        self.settle_escrow();
    }

    /// Everything pinned for the organization, as `set_pinned` took it.
    pub fn pins(&self) -> crate::contacts::Pinned {
        crate::contacts::Pinned {
            root: self.organization_root(),
            escrow: self.pinned(),
        }
    }

    pub fn organization_root(&self) -> Option<EntryHash> {
        *self.organization_root.lock().unwrap()
    }

    pub fn escrows(&self) -> Vec<EscrowKey> {
        self.escrow.lock().unwrap().chain.clone()
    }

    pub fn pinned(&self) -> Option<EscrowKey> {
        self.escrow.lock().unwrap().pinned.clone()
    }

    pub fn unpinned(&self) -> Option<EscrowKey> {
        let escrow = self.escrow.lock().unwrap();
        escrow
            .verified
            .clone()
            .filter(|verified| escrow.pinned.as_ref() != Some(verified))
    }

    fn holds_escrow(&self, escrow: &EscrowKey) -> bool {
        if !self.trusted.lock().unwrap().contains(&escrow.id) {
            return false;
        }
        self.escrow_secret(escrow.id).is_some_and(|secret| {
            secret.public().0 == escrow.public && secret.signing() == escrow.signing
        })
    }

    fn settle_escrow(&self) {
        let (pinned, chain, administrators) = {
            let escrow = self.escrow.lock().unwrap();
            (
                escrow.pinned.clone(),
                escrow.chain.clone(),
                escrow.administrators.clone(),
            )
        };
        let verified = verify_escrow(
            self.organization,
            pinned.as_ref(),
            &chain,
            administrators.as_ref(),
            |escrow| self.holds_escrow(escrow),
        );
        self.escrow.lock().unwrap().verified = verified;
    }

    pub fn escrow(&self) -> Option<EscrowKey> {
        self.escrow.lock().unwrap().verified.clone()
    }

    pub fn escrow_warning(&self) -> Option<EscrowWarning> {
        let escrow = self.escrow.lock().unwrap();
        let latest = escrow.chain.iter().max_by_key(|escrow| escrow.epoch)?;
        match &escrow.verified {
            None => Some(EscrowWarning::Unverified),
            Some(verified) if verified != latest => Some(EscrowWarning::Substituted),
            Some(_) => None,
        }
    }

    pub fn can_make_escrow(&self) -> bool {
        let escrow = self.escrow.lock().unwrap();
        let latest = escrow
            .chain
            .iter()
            .max_by_key(|escrow| escrow.epoch)
            .cloned();
        drop(escrow);
        latest.is_none_or(|latest| self.holds_escrow(&latest))
    }

    pub fn escrow_wrap(&self, key: Uuid) -> Result<Option<WrappedKey>, StoreError> {
        match self.escrow() {
            Some(escrow) => self.escrow_wrap_under(key, &escrow),
            None => Ok(None),
        }
    }

    /// `key` wrapped under `escrow`, unless it is owner-only or already wrapped under it.
    pub fn escrow_wrap_under(
        &self,
        key: Uuid,
        escrow: &EscrowKey,
    ) -> Result<Option<WrappedKey>, StoreError> {
        if self.owner_only(key) {
            return Ok(None);
        }
        if self.wrapped_by(key).contains(&WrappedBy::Key(escrow.id)) {
            return Ok(None);
        }
        let plain = self.carried_plain(key)?;
        Ok(Some(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(escrow.id),
            sender: self.account,
            sealed: RecoveryPublic(escrow.public).seal_anonymous(&plain),
        }))
    }

    /// A new escrow key, chained to the latest, with its secret sealed to this account. It is
    /// not this device's escrow until the server holds it (`adopt_escrow`): a key made
    /// meanwhile and wrapped under it would be refused by a server that does not know it.
    pub fn mint_escrow(&self) -> Result<(EscrowKey, KeySet), StoreError> {
        let previous = {
            let escrow = self.escrow.lock().unwrap();
            escrow
                .chain
                .iter()
                .max_by_key(|escrow| escrow.epoch)
                .cloned()
        };
        // Only a device holding the latest escrow moves the chain on, so no administrator's
        // device makes an epoch that leaves out keys escrowed under one it never opened.
        if let Some(previous) = &previous
            && !self.holds_escrow(previous)
        {
            return Err(StoreError::MissingKey(previous.id));
        }
        let secret = EscrowSecret::generate();
        let mut escrow = EscrowKey {
            id: Uuid::new_v4(),
            epoch: previous.as_ref().map_or(1, |previous| previous.epoch + 1),
            public: secret.public().0,
            signing: secret.signing(),
            maker: self.account,
            signature: Vec::new(),
        };
        let previous = previous.map_or(Uuid::nil(), |previous| previous.id);
        escrow.signature = self
            .identity
            .sign_escrow(&escrow.chained(self.organization, previous));
        self.escrow_secrets
            .lock()
            .unwrap()
            .insert(escrow.id, secret.to_bytes());
        self.trusted.lock().unwrap().insert(escrow.id);
        self.roots
            .lock()
            .unwrap()
            .blocks
            .insert(escrow.id, self.organization);
        let wrapped = vec![self.seal_to(escrow.id, self.account, &self.identity.public())?];
        Ok((
            escrow,
            KeySet {
                keys: Vec::new(),
                wrapped,
            },
        ))
    }

    /// Makes a minted escrow (`mint_escrow`) the latest in the chain, once the server holds it.
    pub fn adopt_escrow(&self, escrow: EscrowKey) {
        let mut chain = self.escrow.lock().unwrap().chain.clone();
        // The key work read meanwhile may have brought it already.
        if chain.iter().any(|held| held.id == escrow.id) {
            return;
        }
        chain.push(escrow);
        self.set_escrows(chain);
    }

    pub fn keys_of_kind(&self, kind: KeyKind) -> Vec<KeyRow> {
        self.rows
            .lock()
            .unwrap()
            .keys
            .values()
            .filter(|row| row.kind == kind)
            .copied()
            .collect()
    }

    pub fn account(&self) -> Uuid {
        self.account
    }

    pub fn identity(&self) -> &RecoverySecret {
        &self.identity
    }

    pub fn ring(&self) -> &Keyring {
        &self.ring
    }

    /// Whether this device holds `key`: a content key in `ring`, or an escrow secret.
    pub fn has(&self, key: Uuid) -> bool {
        self.ring.contains(key) || self.escrow_secrets.lock().unwrap().contains_key(&key)
    }

    pub fn trusted(&self, key: Uuid) -> bool {
        self.trusted.lock().unwrap().contains(&key) && self.has(key)
    }

    pub(crate) fn escrow_secret(&self, escrow: Uuid) -> Option<EscrowSecret> {
        let secret = *self.escrow_secrets.lock().unwrap().get(&escrow)?;
        Some(EscrowSecret::from_bytes(secret))
    }

    pub fn trusted_plain(&self, key: Uuid) -> Result<ContentKey, StoreError> {
        match self.trusted(key) {
            true => self.ring.get(key).ok_or(StoreError::MissingKey(key)),
            false => Err(StoreError::MissingKey(key)),
        }
    }

    /// The block `key` is a key of, as this device minted or opened it (`CarriedKey`, bound
    /// inside the seal), never the server's row: none for a key it cannot place.
    pub fn block_of(&self, key: Uuid) -> Option<Uuid> {
        self.roots.lock().unwrap().blocks.get(&key).copied()
    }

    /// The root this device holds for `block`'s access log: from making it, or carried by a
    /// trusted key of it. None while it holds none, or when its keys carried different roots.
    pub fn root(&self, block: Uuid) -> Option<EntryHash> {
        self.roots.lock().unwrap().pinned.get(&block).copied()
    }

    pub fn root_pin(&self, block: Uuid) -> RootPin {
        let roots = self.roots.lock().unwrap();
        match roots.pinned.get(&block) {
            Some(root) => RootPin::Pinned(*root),
            None if roots.conflicting.contains(&block) => RootPin::Conflicting,
            None => RootPin::Unknown,
        }
    }

    /// Pins the root of a block this device makes, before minting its keys, which carry it.
    pub fn pin_root(&self, block: Uuid, root: EntryHash) {
        self.roots.lock().unwrap().pin(block, root);
    }

    /// `key` as it travels sealed or wrapped (`CarriedKey`): with the block it is a key of
    /// (the organization, for an escrow key), and that block's root only when this device holds
    /// it pinned, so a root it took from nowhere trusted is never passed on as one it vouches
    /// for. A key this device cannot place is not handed on.
    pub fn carried_plain(&self, key: Uuid) -> Result<Vec<u8>, StoreError> {
        let escrow = self.escrow_secrets.lock().unwrap().get(&key).copied();
        let plain = match escrow {
            Some(secret) if self.trusted(key) => secret,
            _ => *self.trusted_plain(key)?.as_bytes(),
        };
        let roots = self.roots.lock().unwrap();
        let block = roots.blocks.get(&key).ok_or(StoreError::MissingKey(key))?;
        Ok(CarriedKey {
            key: plain,
            block: block.into_bytes(),
            root: roots.pinned.get(block).copied(),
        }
        .encode())
    }

    /// The key `bytes` opened to, when they carry the block `row` is a key of. A trusted key
    /// pins the root it carries; any other pins nothing.
    fn opened(&self, row: Option<KeyRow>, bytes: &[u8], vouched: bool) -> Option<Opened> {
        let row = row?;
        let carried = CarriedKey::decode(bytes).ok()?;
        if carried.block != row.block.into_bytes() {
            return None;
        }
        let mut roots = self.roots.lock().unwrap();
        roots.blocks.insert(row.id, row.block);
        if vouched && let Some(root) = carried.root {
            roots.pin(row.block, root);
        }
        Some(Opened {
            bytes: carried.key,
            // Bound inside the seal: an escrow key travels as the organization's, which no
            // block is (its settings block has an id of its own).
            escrow: carried.block == self.organization.into_bytes(),
        })
    }

    pub fn absorb(&self, set: KeySet) -> usize {
        let mut rows = self.rows.lock().unwrap();
        for row in set.keys {
            rows.add_row(row);
        }
        for wrap in set.wrapped {
            rows.add_wrap(wrap);
        }
        let mut changed = 0;
        loop {
            let mut progress = false;
            for (key, wraps) in &rows.wraps {
                if self.trusted(*key) {
                    continue;
                }
                let row = rows.keys.get(key).copied();
                let vouched = wraps
                    .iter()
                    .find_map(|wrap| self.unwrap(wrap, row, true));
                if let Some(opened) = vouched {
                    self.hold(*key, opened);
                    self.trusted.lock().unwrap().insert(*key);
                    changed += 1;
                    progress = true;
                    continue;
                }
                if self.has(*key) {
                    continue;
                }
                if let Some(bytes) = wraps
                    .iter()
                    .find_map(|wrap| self.unwrap(wrap, row, false))
                {
                    self.hold(*key, bytes);
                    changed += 1;
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        drop(rows);
        self.settle_escrow();
        changed
    }

    /// Keeps an opened key: an escrow's secret apart from the content keys.
    fn hold(&self, key: Uuid, opened: Opened) {
        match opened.escrow {
            true => {
                self.escrow_secrets.lock().unwrap().insert(key, opened.bytes);
            }
            false => self.ring.insert(key, ContentKey::from_bytes(opened.bytes)),
        }
    }

    fn unwrap(
        &self,
        wrap: &WrappedKey,
        row: Option<KeyRow>,
        vouched: bool,
    ) -> Option<Opened> {
        let block = row.map(|row| row.block);
        let bytes = match wrap.wrapped_by {
            WrappedBy::Account(account) if account == self.account => {
                if !self.accepts(wrap.sender, block, vouched) {
                    return None;
                }
                let sender = Party {
                    account: wrap.sender.into_bytes(),
                    public: self.sender(wrap.sender)?,
                };
                self.identity
                    .open_from(
                        &sender,
                        self.account.into_bytes(),
                        wrap.key.as_bytes(),
                        &wrap.sealed,
                    )
                    .ok()?
            }
            WrappedBy::Account(_) => return None,
            WrappedBy::Key(by) if self.escrow_secret(by).is_some() => {
                if vouched {
                    return None;
                }
                self.escrow_secret(by)?.open(&wrap.sealed).ok()?
            }
            WrappedBy::Key(by) => {
                if be_block_store::sealed_key(&wrap.sealed).ok()? != by || !self.ring.contains(by) {
                    return None;
                }
                if vouched && !self.trusted(by) {
                    return None;
                }
                self.ring.open(&wrap.sealed).ok()?
            }
        };
        self.opened(row, &bytes, vouched)
    }

    pub fn row(&self, key: Uuid) -> Option<KeyRow> {
        self.rows.lock().unwrap().keys.get(&key).copied()
    }

    pub fn keys_of(&self, block: Uuid, kind: KeyKind) -> Vec<KeyRow> {
        let rows = self.rows.lock().unwrap();
        let mut found: Vec<KeyRow> = rows
            .by_block
            .get(&block)
            .into_iter()
            .flatten()
            .filter_map(|key| rows.keys.get(key))
            .filter(|row| row.kind == kind)
            .copied()
            .collect();
        found.sort_by_key(|row| row.epoch);
        found
    }

    pub fn scope_key(&self, block: Uuid) -> Option<Uuid> {
        self.keys_of(block, KeyKind::Scope)
            .into_iter()
            .rev()
            .map(|row| row.id)
            .find(|key| self.ring.contains(*key))
    }

    pub fn wrapped_by(&self, key: Uuid) -> Vec<WrappedBy> {
        self.rows
            .lock()
            .unwrap()
            .wraps
            .get(&key)
            .into_iter()
            .flatten()
            .map(|wrap| wrap.wrapped_by)
            .collect()
    }

    pub fn wrap(&self, key: Uuid, under: Uuid) -> Result<WrappedKey, StoreError> {
        let plain = self.carried_plain(key)?;
        self.trusted_plain(under)?;
        Ok(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(under),
            sender: self.account,
            sealed: self.ring.seal(under, &plain)?,
        })
    }

    pub fn seal_to(
        &self,
        key: Uuid,
        account: Uuid,
        public: &RecoveryPublic,
    ) -> Result<WrappedKey, StoreError> {
        let plain = self.carried_plain(key)?;
        let to = Party {
            account: account.into_bytes(),
            public: *public,
        };
        Ok(WrappedKey {
            key,
            wrapped_by: WrappedBy::Account(account),
            sender: self.account,
            sealed: self
                .identity
                .seal_for(self.account.into_bytes(), &to, key.as_bytes(), &plain),
        })
    }

    fn mint(&self, block: Uuid, kind: KeyKind, owner_only: bool) -> KeyRow {
        let epoch = self
            .keys_of(block, kind)
            .last()
            .map_or(1, |row| row.epoch + 1);
        let row = KeyRow {
            id: match owner_only {
                true => be_block_protocol::owner_only_key_id(),
                false => Uuid::new_v4(),
            },
            block,
            kind,
            epoch,
        };
        self.ring.insert(row.id, ContentKey::random());
        self.trusted.lock().unwrap().insert(row.id);
        self.roots.lock().unwrap().blocks.insert(row.id, block);
        row
    }

    pub fn mint_data(&self, block: Uuid, under: Uuid) -> Result<(Uuid, KeySet), StoreError> {
        self.trusted_plain(under)?;
        let row = self.mint(block, KeyKind::Data, false);
        let wrapped = self.wrap(row.id, under)?;
        Ok((
            row.id,
            KeySet {
                keys: vec![row],
                wrapped: vec![wrapped],
            },
        ))
    }

    pub fn mint_scope(
        &self,
        block: Uuid,
        parent_scope: Option<Uuid>,
    ) -> Result<(Uuid, KeySet), StoreError> {
        if let Some(parent) = parent_scope {
            self.trusted_plain(parent)?;
        }
        let row = self.mint(block, KeyKind::Scope, false);
        let mut wrapped = vec![self.seal_to(row.id, self.account, &self.identity.public())?];
        if let Some(parent) = parent_scope {
            wrapped.push(self.wrap(row.id, parent)?);
        }
        wrapped.extend(self.escrow_wrap(row.id)?);
        Ok((
            row.id,
            KeySet {
                keys: vec![row],
                wrapped,
            },
        ))
    }

    /// The scope key of an owner-only block, sealed to this account alone: never wrapped under
    /// the scope above nor escrowed.
    pub fn mint_own_scope(&self, block: Uuid) -> Result<(Uuid, KeySet), StoreError> {
        let row = self.mint(block, KeyKind::Scope, true);
        let wrapped = vec![self.seal_to(row.id, self.account, &self.identity.public())?];
        Ok((
            row.id,
            KeySet {
                keys: vec![row],
                wrapped,
            },
        ))
    }

    /// Whether `key` is an owner-only block's scope key, which nothing else may wrap. Told by
    /// the key's own id, which every seal of it binds, so the server cannot say otherwise.
    pub fn owner_only(&self, key: Uuid) -> bool {
        be_block_protocol::owner_only_key(key)
    }

    /// Moves on each time keys are recorded (`record`).
    pub fn revision(&self) -> u64 {
        self.rows.lock().unwrap().revision
    }

    pub fn record(&self, set: &KeySet) {
        let mut rows = self.rows.lock().unwrap();
        rows.revision += 1;
        for row in &set.keys {
            rows.add_row(*row);
        }
        for wrap in &set.wrapped {
            rows.add_wrap(wrap.clone());
        }
    }

    pub fn missing(&self, keys: impl IntoIterator<Item = Uuid>) -> HashSet<Uuid> {
        keys.into_iter()
            .filter(|key| !self.has(*key))
            .collect()
    }
}
