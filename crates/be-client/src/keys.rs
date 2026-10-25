use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use be_keys::{EscrowSecret, Party, RecoveryPublic, RecoverySecret};
use be_protocol::{EscrowKey, KeyKind, KeyRow, KeySet, WrappedBy, WrappedKey};
use be_store::{ContentKey, Keyring, StoreError};
use uuid::Uuid;

pub struct KeyBook {
    account: Uuid,
    organization: Uuid,
    identity: RecoverySecret,
    ring: Keyring,
    rows: Mutex<Rows>,
    contacts: Mutex<HashMap<Uuid, RecoveryPublic>>,
    trusted: Mutex<HashSet<Uuid>>,
    escrow: Mutex<Escrow>,
    server: Mutex<Option<(Uuid, RecoveryPublic)>>,
}

#[derive(Default)]
struct Rows {
    keys: HashMap<Uuid, KeyRow>,
    wraps: HashMap<Uuid, Vec<WrappedKey>>,
    by_block: HashMap<Uuid, Vec<Uuid>>,
}

#[derive(Default)]
struct Escrow {
    chain: Vec<EscrowKey>,
    pinned: Option<EscrowKey>,
    verified: Option<EscrowKey>,
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

pub fn verify_escrow(
    organization: Uuid,
    pinned: Option<&EscrowKey>,
    chain: &[EscrowKey],
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
        if let Some(known) = &verified
            && newer
            && be_keys::verify_signature(
                &known.signing,
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
            rows: Mutex::new(Rows::default()),
            contacts: Mutex::new(HashMap::new()),
            trusted: Mutex::new(HashSet::new()),
            escrow: Mutex::new(Escrow::default()),
            server: Mutex::new(None),
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

    pub fn contact(&self, account: Uuid) -> Option<RecoveryPublic> {
        if let Some((server, public)) = self.server()
            && server == account
        {
            return Some(public);
        }
        match account == self.account {
            true => Some(self.identity.public()),
            false => self.contacts.lock().unwrap().get(&account).copied(),
        }
    }

    fn accepts(&self, sender: Uuid, block: Option<Uuid>, vouched: bool) -> bool {
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

    pub fn set_pinned(&self, pinned: Option<EscrowKey>) {
        self.escrow.lock().unwrap().pinned = pinned;
        self.settle_escrow();
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
        self.ring.get(escrow.id).is_some_and(|secret| {
            let secret = EscrowSecret::from_bytes(*secret.as_bytes());
            secret.public().0 == escrow.public && secret.signing() == escrow.signing
        })
    }

    fn settle_escrow(&self) {
        let (pinned, chain) = {
            let escrow = self.escrow.lock().unwrap();
            (escrow.pinned.clone(), escrow.chain.clone())
        };
        let verified = verify_escrow(self.organization, pinned.as_ref(), &chain, |escrow| {
            self.holds_escrow(escrow)
        });
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
        let Some(escrow) = self.escrow() else {
            return Ok(None);
        };
        if self.wrapped_by(key).contains(&WrappedBy::Key(escrow.id)) {
            return Ok(None);
        }
        let plain = self.trusted_plain(key)?;
        Ok(Some(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(escrow.id),
            sender: self.account,
            sealed: RecoveryPublic(escrow.public).seal_anonymous(plain.as_bytes()),
        }))
    }

    pub fn mint_escrow(&self) -> Result<(EscrowKey, KeySet), StoreError> {
        let previous = {
            let escrow = self.escrow.lock().unwrap();
            escrow
                .chain
                .iter()
                .max_by_key(|escrow| escrow.epoch)
                .cloned()
        };
        let signer = match &previous {
            Some(previous) if self.holds_escrow(previous) => Some((
                previous.clone(),
                EscrowSecret::from_bytes(
                    *self
                        .ring
                        .get(previous.id)
                        .ok_or(StoreError::MissingKey(previous.id))?
                        .as_bytes(),
                ),
            )),
            Some(previous) => return Err(StoreError::MissingKey(previous.id)),
            None => None,
        };
        let secret = EscrowSecret::generate();
        let mut escrow = EscrowKey {
            id: Uuid::new_v4(),
            epoch: previous.as_ref().map_or(1, |previous| previous.epoch + 1),
            public: secret.public().0,
            signing: secret.signing(),
            signature: Vec::new(),
        };
        if let Some((previous, signer)) = signer {
            escrow.signature = signer.sign(&escrow.chained(self.organization, previous.id));
        }
        self.ring
            .insert(escrow.id, ContentKey::from_bytes(secret.to_bytes()));
        self.trusted.lock().unwrap().insert(escrow.id);
        let mut chain = self.escrow.lock().unwrap().chain.clone();
        chain.push(escrow.clone());
        self.set_escrows(chain);
        let wrapped = vec![self.seal_to(escrow.id, self.account, &self.identity.public())?];
        Ok((
            escrow,
            KeySet {
                keys: Vec::new(),
                wrapped,
            },
        ))
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

    pub fn has(&self, key: Uuid) -> bool {
        self.ring.contains(key)
    }

    pub fn trusted(&self, key: Uuid) -> bool {
        self.trusted.lock().unwrap().contains(&key) && self.ring.contains(key)
    }

    pub fn trusted_plain(&self, key: Uuid) -> Result<ContentKey, StoreError> {
        match self.trusted(key) {
            true => self.ring.get(key).ok_or(StoreError::MissingKey(key)),
            false => Err(StoreError::MissingKey(key)),
        }
    }

    pub fn absorb(&self, set: KeySet) -> usize {
        let mut rows = self.rows.lock().unwrap();
        for row in set.keys {
            rows.add_row(row);
        }
        for wrap in set.wrapped {
            rows.add_wrap(wrap);
        }
        let escrows: HashSet<Uuid> = rows
            .keys
            .values()
            .filter(|row| row.kind == KeyKind::Escrow)
            .map(|row| row.id)
            .collect();
        let mut changed = 0;
        loop {
            let mut progress = false;
            for (key, wraps) in &rows.wraps {
                if self.trusted(*key) {
                    continue;
                }
                let block = rows.keys.get(key).map(|row| row.block);
                let vouched = wraps
                    .iter()
                    .find_map(|wrap| self.unwrap(wrap, block, &escrows, true));
                if let Some(bytes) = vouched {
                    self.ring.insert(*key, ContentKey::from_bytes(bytes));
                    self.trusted.lock().unwrap().insert(*key);
                    changed += 1;
                    progress = true;
                    continue;
                }
                if self.ring.contains(*key) {
                    continue;
                }
                if let Some(bytes) = wraps
                    .iter()
                    .find_map(|wrap| self.unwrap(wrap, block, &escrows, false))
                {
                    self.ring.insert(*key, ContentKey::from_bytes(bytes));
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

    fn unwrap(
        &self,
        wrap: &WrappedKey,
        block: Option<Uuid>,
        escrows: &HashSet<Uuid>,
        vouched: bool,
    ) -> Option<[u8; 32]> {
        let bytes = match wrap.wrapped_by {
            WrappedBy::Account(account) if account == self.account => {
                if !self.accepts(wrap.sender, block, vouched) {
                    return None;
                }
                let sender = Party {
                    account: wrap.sender.into_bytes(),
                    public: self.contact(wrap.sender)?,
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
            WrappedBy::Key(by) if escrows.contains(&by) => {
                if vouched {
                    return None;
                }
                let secret = self.ring.get(by)?;
                EscrowSecret::from_bytes(*secret.as_bytes())
                    .open(&wrap.sealed)
                    .ok()?
            }
            WrappedBy::Key(by) => {
                if be_store::sealed_key(&wrap.sealed).ok()? != by || !self.ring.contains(by) {
                    return None;
                }
                if vouched && !self.trusted(by) {
                    return None;
                }
                self.ring.open(&wrap.sealed).ok()?
            }
        };
        bytes.try_into().ok()
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
        let plain = self.trusted_plain(key)?;
        self.trusted_plain(under)?;
        Ok(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(under),
            sender: self.account,
            sealed: self.ring.seal(under, plain.as_bytes())?,
        })
    }

    pub fn seal_to(
        &self,
        key: Uuid,
        account: Uuid,
        public: &RecoveryPublic,
    ) -> Result<WrappedKey, StoreError> {
        let plain = self.trusted_plain(key)?;
        let to = Party {
            account: account.into_bytes(),
            public: *public,
        };
        Ok(WrappedKey {
            key,
            wrapped_by: WrappedBy::Account(account),
            sender: self.account,
            sealed: self.identity.seal_for(
                self.account.into_bytes(),
                &to,
                key.as_bytes(),
                plain.as_bytes(),
            ),
        })
    }

    fn mint(&self, block: Uuid, kind: KeyKind) -> KeyRow {
        let epoch = self
            .keys_of(block, kind)
            .last()
            .map_or(1, |row| row.epoch + 1);
        let row = KeyRow {
            id: Uuid::new_v4(),
            block,
            kind,
            epoch,
        };
        self.ring.insert(row.id, ContentKey::random());
        self.trusted.lock().unwrap().insert(row.id);
        row
    }

    pub fn mint_data(&self, block: Uuid, under: Uuid) -> Result<(Uuid, KeySet), StoreError> {
        self.trusted_plain(under)?;
        let row = self.mint(block, KeyKind::Data);
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
        let row = self.mint(block, KeyKind::Scope);
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

    pub fn record(&self, set: &KeySet) {
        let mut rows = self.rows.lock().unwrap();
        for row in &set.keys {
            rows.add_row(*row);
        }
        for wrap in &set.wrapped {
            rows.add_wrap(wrap.clone());
        }
    }

    pub fn missing(&self, keys: impl IntoIterator<Item = Uuid>) -> HashSet<Uuid> {
        keys.into_iter()
            .filter(|key| !self.ring.contains(*key))
            .collect()
    }
}
