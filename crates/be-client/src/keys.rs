use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use be_keys::{RecoveryPublic, RecoverySecret};
use be_protocol::{EscrowKey, KeyKind, KeyRow, KeySet, WrappedBy, WrappedKey};
use be_store::{ContentKey, Keyring, StoreError};
use uuid::Uuid;

pub struct KeyBook {
    account: Uuid,
    identity: RecoverySecret,
    ring: Keyring,
    rows: Mutex<Rows>,
    contacts: Mutex<HashMap<Uuid, RecoveryPublic>>,
    escrow: Mutex<Option<EscrowKey>>,
}

#[derive(Default)]
struct Rows {
    keys: HashMap<Uuid, KeyRow>,
    wraps: HashMap<Uuid, Vec<WrappedKey>>,
    by_block: HashMap<Uuid, Vec<Uuid>>,
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

impl KeyBook {
    pub fn new(account: Uuid, identity: RecoverySecret, ring: Keyring) -> Self {
        Self {
            account,
            identity,
            ring,
            rows: Mutex::new(Rows::default()),
            contacts: Mutex::new(HashMap::new()),
            escrow: Mutex::new(None),
        }
    }

    pub fn set_contacts(&self, verified: impl IntoIterator<Item = (Uuid, RecoveryPublic)>) {
        *self.contacts.lock().unwrap() = verified.into_iter().collect();
    }

    pub fn contact(&self, account: Uuid) -> Option<RecoveryPublic> {
        match account == self.account {
            true => Some(self.identity.public()),
            false => self.contacts.lock().unwrap().get(&account).copied(),
        }
    }

    pub fn set_escrow(&self, escrow: Option<EscrowKey>) {
        if let Some(escrow) = escrow {
            let mut rows = self.rows.lock().unwrap();
            if !rows.keys.contains_key(&escrow.id) {
                rows.add_row(KeyRow {
                    id: escrow.id,
                    block: Uuid::nil(),
                    kind: KeyKind::Escrow,
                    epoch: escrow.epoch,
                });
            }
        }
        *self.escrow.lock().unwrap() = escrow;
    }

    pub fn escrow(&self) -> Option<EscrowKey> {
        *self.escrow.lock().unwrap()
    }

    pub fn escrow_wrap(&self, key: Uuid) -> Result<Option<WrappedKey>, StoreError> {
        let Some(escrow) = self.escrow() else {
            return Ok(None);
        };
        let plain = self.ring.get(key).ok_or(StoreError::MissingKey(key))?;
        Ok(Some(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(escrow.id),
            sealed: RecoveryPublic(escrow.public).seal(plain.as_bytes()),
        }))
    }

    pub fn mint_escrow(&self, organization: Uuid) -> Result<(EscrowKey, KeySet), StoreError> {
        let epoch = self
            .keys_of(organization, KeyKind::Escrow)
            .last()
            .map_or(1, |row| row.epoch + 1)
            .max(self.escrow().map_or(1, |escrow| escrow.epoch + 1));
        let secret = ContentKey::random();
        let escrow = EscrowKey {
            id: Uuid::new_v4(),
            epoch,
            public: RecoverySecret::from_bytes(*secret.as_bytes()).public().0,
        };
        self.ring.insert(escrow.id, secret);
        self.rows.lock().unwrap().add_row(KeyRow {
            id: escrow.id,
            block: organization,
            kind: KeyKind::Escrow,
            epoch,
        });
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
        let mut opened = 0;
        loop {
            let mut progress = false;
            for (key, wraps) in &rows.wraps {
                if self.ring.contains(*key) {
                    continue;
                }
                if let Some(bytes) = wraps.iter().find_map(|wrap| self.unwrap(wrap, &escrows)) {
                    self.ring.insert(*key, ContentKey::from_bytes(bytes));
                    opened += 1;
                    progress = true;
                }
            }
            if !progress {
                return opened;
            }
        }
    }

    fn unwrap(&self, wrap: &WrappedKey, escrows: &HashSet<Uuid>) -> Option<[u8; 32]> {
        let bytes = match wrap.wrapped_by {
            WrappedBy::Account(account) if account == self.account => {
                self.identity.open(&wrap.sealed).ok()?
            }
            WrappedBy::Account(_) => return None,
            WrappedBy::Key(by) if escrows.contains(&by) => {
                let secret = self.ring.get(by)?;
                RecoverySecret::from_bytes(*secret.as_bytes())
                    .open(&wrap.sealed)
                    .ok()?
            }
            WrappedBy::Key(by) => {
                if be_store::sealed_key(&wrap.sealed).ok()? != by || !self.ring.contains(by) {
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
        let plain = self.ring.get(key).ok_or(StoreError::MissingKey(key))?;
        Ok(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(under),
            sealed: self.ring.seal(under, plain.as_bytes())?,
        })
    }

    pub fn seal_to(
        &self,
        key: Uuid,
        account: Uuid,
        public: &RecoveryPublic,
    ) -> Result<WrappedKey, StoreError> {
        let plain = self.ring.get(key).ok_or(StoreError::MissingKey(key))?;
        Ok(WrappedKey {
            key,
            wrapped_by: WrappedBy::Account(account),
            sealed: public.seal(plain.as_bytes()),
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
        row
    }

    pub fn mint_data(&self, block: Uuid, under: Uuid) -> Result<(Uuid, KeySet), StoreError> {
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
