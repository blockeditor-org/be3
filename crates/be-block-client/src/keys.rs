use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use be_block_keys::{
    CarriedKey, Checked, EntryHash, EscrowSecret, Heads, Level, Party, PublicIdentity,
    RecoveryPublic, RecoverySecret, SigningPublic,
};
use be_block_protocol::{EscrowKey, KeyKind, KeyRow, KeySet, WrappedBy, WrappedKey};
use be_block_store::{ContentKey, Keyring, StoreError};
use uuid::Uuid;

/// Its locks are taken in one order, `rows`, then `roots` and `trusted`, then `server_held` and
/// `under_owner_only`, then `escrow` and `escrowed`, then `escrow_secrets`, then `ring`'s own:
/// `absorb` holds `rows` while it opens keys, so nothing that holds a later lock takes an earlier
/// one.
pub struct KeyBook {
    account: Uuid,
    organization: Uuid,
    identity: RecoverySecret,
    ring: Keyring,
    /// The escrow secrets this device holds, by escrow key id: X25519 secrets, never content
    /// keys, so kept out of `ring`, where an object whose header named one would open with it.
    escrow_secrets: Mutex<HashMap<Uuid, [u8; 32]>>,
    /// Keys opened only through a key the server holds (`server_held`), beside the settings
    /// block's own.
    server_held: Mutex<HashSet<Uuid>>,
    /// Keys under an owner-only scope key, minted or opened through one (`owner_only_covers`).
    under_owner_only: Mutex<HashSet<Uuid>>,
    rows: Mutex<Rows>,
    contacts: Mutex<HashMap<Uuid, RecoveryPublic>>,
    trusted: Mutex<HashSet<Uuid>>,
    escrow: Mutex<Escrow>,
    server: Mutex<Option<(Uuid, RecoveryPublic)>>,
    roots: Mutex<Roots>,
    /// Keys opened from an escrow wrap, waiting for `Peer::settle_escrowed` to find their sender
    /// granted Edit or above in the block's log before they are taken.
    escrowed: Mutex<HashMap<Uuid, (Escrowed, [u8; 32])>>,
    /// The hash of the organization's access log root, pinned from making it or an
    /// invitation (`crate::contacts::Pinned`).
    organization_root: Mutex<Option<EntryHash>>,
    /// What a key opening here tells (`on_opened`): the one place every path that opens a key
    /// (a fetch, a contact verified, escrow) signals that keys arrived.
    on_opened: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

/// A key opened from what was sealed or wrapped, and whether it is an escrow's secret.
struct Opened {
    bytes: [u8; 32],
    /// The block it is a key of, as the seal carried it.
    block: Uuid,
    escrow: bool,
    /// Whether it opened only through a key the server holds.
    through_server: bool,
    /// Whether it opened through an owner-only block's key (`owner_only_covers`).
    under_owner_only: bool,
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
    /// Roots taken only from a member's escrow wrap, for a block this device pinned nothing
    /// else for: as good as that member, so never passed on (`carried_plain` reads `pinned`
    /// alone), never shown as verified, and pinned in full only once a trusted key carries the
    /// same root.
    weak: HashMap<Uuid, EntryHash>,
    /// The log heads each key's epoch was made under (`be_block_keys::Heads`), as this device
    /// minted it or as the trusted key that first opened carried them: passed on with it.
    heads: HashMap<Uuid, Heads>,
    /// Each block's log as this device last checked it (`KeyBook::note_log`): the heads a key
    /// minted here names, and what a contact's seal is checked against (`sender_may`).
    logs: HashMap<Uuid, Checked>,
    /// The entries of each of `logs`, root first: what a stale answer is replaced by
    /// (`KeyBook::served`).
    entries: HashMap<Uuid, Vec<be_block_keys::Step>>,
    /// The keys each key this device took through a wrap under another key was opened
    /// through, every one: an epoch opened through an outdated one is outdated too
    /// (`KeyBook::outdated`).
    through: HashMap<Uuid, Vec<Uuid>>,
    /// How far this device last checked each scope's log, kept across restarts with the refs
    /// (`crate::refs::KeptLog`): a log served later that does not reach it is withheld, and
    /// with no log read, an epoch is judged by it.
    kept: HashMap<Uuid, crate::refs::KeptLog>,
    /// Blocks this device set out to make whose creation no server has taken yet, with the root
    /// it made (`KeyBook::pin_root`) and any other root a key of them carried meanwhile: a
    /// creation the server refuses as made by someone else lets its root go, and the other root
    /// stands (`KeyBook::forget_made`).
    made: HashMap<Uuid, Made>,
    /// Roots this device made for blocks whose creation no server took (`KeyBook::forget_made`),
    /// kept across restarts (`refs::Refs::unmade`): the keys that creation minted still open
    /// and carry it, but it is pinned no more, so a block someone else made under the id keeps
    /// its own root.
    unmade: HashMap<Uuid, EntryHash>,
}

/// A root this device made for a block no server has taken yet (`Roots::made`).
struct Made {
    root: EntryHash,
    /// The other roots keys of the block carried meanwhile, and whether each was taken weakly.
    others: Vec<(EntryHash, bool)>,
}

/// What a device holds of a block's access log root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootPin {
    /// Pinned from making the block or from a trusted key of it.
    Pinned(EntryHash),
    /// Taken only from a member's escrow wrap (`Roots::weak`): a log checked from it shows what
    /// that member, and the server, say, unverified.
    Weak(EntryHash),
    /// Trusted keys of it carried different roots: one was forged.
    Conflicting,
    /// None yet.
    Unknown,
}

impl Roots {
    /// `key`, a key of `block`'s scope, and every key it was opened through, each with the
    /// scope it is a key of, up every path (`through`).
    /// None when the path is longer than any honest one (`MAX_PATH`), or passes through a key
    /// this device cannot place: either way it is judged outdated.
    fn path(&self, key: Uuid, block: Uuid) -> Option<Vec<(Uuid, Uuid)>> {
        let mut found = vec![(key, block)];
        let mut at = 0;
        // A cycle would be a forged one, cut by `found`.
        while at < found.len() {
            let (key, _) = found[at];
            for by in self.through.get(&key).into_iter().flatten() {
                if found.iter().any(|(seen, _)| seen == by) {
                    continue;
                }
                let scope = self.blocks.get(by)?;
                if found.len() >= MAX_PATH {
                    return None;
                }
                found.push((*by, *scope));
            }
            at += 1;
        }
        Some(found)
    }

    /// Notes `root`, carried by a key of `block`, when it is not the root this device made for
    /// it while that creation waits (`made`).
    fn carried_other(&mut self, block: Uuid, root: EntryHash, weak: bool) {
        if let Some(made) = self.made.get_mut(&block)
            && made.root != root
            && !made.others.contains(&(root, weak))
        {
            made.others.push((root, weak));
        }
    }

    fn conflict(&mut self, block: Uuid) {
        self.pinned.remove(&block);
        self.weak.remove(&block);
        self.conflicting.insert(block);
    }

    fn pin(&mut self, block: Uuid, root: EntryHash) {
        if self.unmade.get(&block) == Some(&root) {
            return;
        }
        self.carried_other(block, root, false);
        if self.conflicting.contains(&block) {
            return;
        }
        match self.weak.remove(&block) {
            Some(weak) if weak != root => return self.conflict(block),
            _ => {}
        }
        match self.pinned.get(&block) {
            Some(pinned) if *pinned != root => self.conflict(block),
            Some(_) => {}
            None => {
                self.pinned.insert(block, root);
            }
        }
    }

    /// Takes `root` weakly (`Roots::weak`) for a block pinned nothing else for; one that
    /// disagrees with what is held is a conflict.
    fn pin_weak(&mut self, block: Uuid, root: EntryHash) {
        if self.unmade.get(&block) == Some(&root) {
            return;
        }
        self.carried_other(block, root, true);
        if self.conflicting.contains(&block) {
            return;
        }
        let held = self.pinned.get(&block).or(self.weak.get(&block)).copied();
        match held {
            Some(held) if held != root => self.conflict(block),
            Some(_) => {}
            None => {
                self.weak.insert(block, root);
            }
        }
    }
}

/// What a served log is to this device (`KeyBook::served`).
pub enum Served {
    Take,
    /// Short of a log read further in this run: that one stands, with its entries.
    Stale(Box<(Checked, Vec<be_block_keys::Step>)>),
    /// Short of what an earlier run kept: the server holds history back.
    Withheld,
}

/// The most keys a key is judged through (`Roots::path`): one per scope above it, and more
/// than any honest tree holds. A longer path is judged outdated.
pub const MAX_PATH: usize = 256;

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
    /// Its members (administrators among them), with the keys its log names: those an escrow
    /// wrap is opened from.
    members: HashMap<Uuid, PublicIdentity>,
    /// The log itself as last checked: how far it reaches (an epoch is taken only once it
    /// reaches the head the epoch names) and who administered it as of each entry.
    log: Option<Checked>,
    /// The epoch this device verified that the log outdates (a demotion since), wrapped under
    /// no more until the next.
    outdated: Option<EscrowKey>,
}

/// A key opened from an escrow wrap, before it is taken: its block, the member who sealed it,
/// and the root of the block's log it carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Escrowed {
    pub key: Uuid,
    pub block: Uuid,
    pub sender: Uuid,
    pub root: Option<EntryHash>,
    /// The log heads its epoch was made under, as the wrap carries them.
    pub heads: Heads,
    /// From an escrow wrap; else sealed to this account by a member who is not a contact,
    /// which is never trusted.
    pub escrow: bool,
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
/// this device holds it) and that log reaches the head the epoch names (`holds`); an epoch this
/// account made (`held`) is taken without. A demoted
/// administrator, still holding the escrow keys of their time, signs no epoch a device takes,
/// nor does the server; and an epoch made after a demotion is taken only by a device that
/// holds the demotion, so a server that withholds it gets no epoch taken past it.
pub fn verify_escrow(
    organization: Uuid,
    account: Uuid,
    pinned: Option<&EscrowKey>,
    chain: &[EscrowKey],
    administrators: Option<&HashMap<Uuid, SigningPublic>>,
    holds: impl Fn(&EntryHash) -> bool,
    held: impl Fn(&EscrowKey) -> bool,
) -> Option<EscrowKey> {
    let mut sorted: Vec<&EscrowKey> = chain.iter().collect();
    sorted.sort_by_key(|escrow| escrow.epoch);
    let mut verified = pinned.cloned();
    for next in sorted {
        let newer = verified
            .as_ref()
            .is_none_or(|known| next.epoch > known.epoch);
        // An epoch this account made itself is taken as held; any other only through the signed
        // chain, as a secret sealed by a demoted administrator proves nothing.
        if next.maker == account
            && held(next)
            && (newer || verified.as_ref().is_some_and(|known| known.id == next.id))
        {
            verified = Some(next.clone());
            continue;
        }
        let maker = administrators.and_then(|administrators| administrators.get(&next.maker));
        if let Some(known) = &verified
            && newer
            && next.epoch == known.epoch + 1
            && holds(&next.head)
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
            server_held: Mutex::default(),
            under_owner_only: Mutex::default(),
            rows: Mutex::new(Rows::default()),
            contacts: Mutex::new(HashMap::new()),
            trusted: Mutex::new(HashSet::new()),
            escrow: Mutex::new(Escrow::default()),
            server: Mutex::new(None),
            roots: Mutex::new(Roots::default()),
            escrowed: Mutex::new(HashMap::new()),
            organization_root: Mutex::new(None),
            on_opened: Mutex::new(None),
        }
    }

    /// Has `told` run whenever a key from elsewhere opens here (`absorb`, `take_escrowed`), once
    /// the book lets go of its locks: what was waiting on a key reads or submits again.
    pub(crate) fn on_opened(&self, told: impl Fn() + Send + Sync + 'static) {
        *self.on_opened.lock().unwrap() = Some(Box::new(told));
    }

    fn told_opened(&self) {
        if let Some(told) = &*self.on_opened.lock().unwrap() {
            told();
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

    /// The organization as its checked log names it: its administrators with their signing
    /// keys, and its members (administrators among them) with their keys. `None` when the log
    /// did not check.
    pub fn set_organization(&self, checked: Option<&be_block_keys::Checked>) {
        {
            let mut escrow = self.escrow.lock().unwrap();
            escrow.log = checked.cloned();
            escrow.administrators = checked.map(|checked| {
                checked
                    .administrators
                    .iter()
                    .map(|(account, identity)| (Uuid::from_bytes(*account), identity.signing))
                    .collect()
            });
            escrow.members = checked
                .map(|checked| {
                    checked
                        .administrators
                        .iter()
                        .map(|(account, identity)| (*account, *identity))
                        .chain(
                            checked
                                .members
                                .iter()
                                .map(|(account, named)| (*account, named.identity)),
                        )
                        .map(|(account, identity)| (Uuid::from_bytes(account), identity))
                        .collect()
                })
                .unwrap_or_default();
        }
        self.settle_escrow();
    }

    /// Whether `set` holds an escrow wrap, under an escrow key this device holds, from a sender
    /// not among the members it knows: the organization's log is read again first.
    pub fn escrow_senders_unknown(&self, set: &KeySet) -> bool {
        let escrow = self.escrow.lock().unwrap();
        set.wrapped.iter().any(|wrap| match wrap.wrapped_by {
            WrappedBy::Key(by) => {
                escrow.chain.iter().any(|held| held.id == by)
                    && self.escrow_secret(by).is_some()
                    && !escrow.members.contains_key(&wrap.sender)
            }
            WrappedBy::Account(account) => {
                account == self.account
                    && !escrow.members.contains_key(&wrap.sender)
                    && self.sender(wrap.sender).is_none()
            }
        })
    }

    /// Whether the organization's checked log names `account` an administrator.
    pub fn is_administrator(&self, account: Uuid) -> bool {
        self.escrow
            .lock()
            .unwrap()
            .administrators
            .as_ref()
            .is_some_and(|administrators| administrators.contains_key(&account))
    }

    /// The keys opened from escrow wraps that wait for their sender's grant to be checked.
    pub fn escrowed(&self) -> Vec<Escrowed> {
        let mut found: Vec<Escrowed> = self
            .escrowed
            .lock()
            .unwrap()
            .values()
            .map(|(escrowed, _)| *escrowed)
            .collect();
        found.sort_by_key(|escrowed| escrowed.key);
        found
    }

    /// Drops a key opened from an escrow wrap whose carried root disagrees with the root this
    /// device pinned for its block, and marks the block's root conflicting
    /// (`RootPin::Conflicting`), so a planted root shows rather than quietly holding back the
    /// genuine wraps.
    pub fn conflicting_escrowed(&self, key: Uuid, root: EntryHash) {
        let Some((escrowed, _)) = self.escrowed.lock().unwrap().remove(&key) else {
            return;
        };
        self.roots.lock().unwrap().pin(escrowed.block, root);
    }

    /// Takes a key opened from an escrow wrap, or sealed by a member who is not a contact, once
    /// its sender is found granted Edit or above in the block's log rooted at `root`. `weak`
    /// when that root came from the wrap alone, from a member: it is taken weakly
    /// (`Roots::weak`) and the key untrusted, so it vouches for nothing, pins nothing it opens,
    /// and is handed on by no one; a key a non-contact sealed directly is untrusted always.
    pub fn take_escrowed(&self, key: Uuid, root: EntryHash, weak: bool) {
        let escrow = self
            .escrowed
            .lock()
            .unwrap()
            .get(&key)
            .is_some_and(|(escrowed, _)| escrowed.escrow);
        let Some((escrowed, secret)) = self.escrowed.lock().unwrap().remove(&key) else {
            return;
        };
        {
            let mut roots = self.roots.lock().unwrap();
            roots.blocks.insert(key, escrowed.block);
            match weak {
                true => roots.pin_weak(escrowed.block, root),
                false => roots.pin(escrowed.block, root),
            }
            roots.heads.entry(key).or_insert(escrowed.heads);
        }
        self.ring
            .insert(key, ContentKey::from_bytes(secret), escrowed.block);
        // A key a member who is no contact sealed to this account is read, never vouched by.
        if !weak && escrow {
            self.trusted.lock().unwrap().insert(key);
        }
        self.told_opened();
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
        let (pinned, chain, administrators, log) = {
            let escrow = self.escrow.lock().unwrap();
            (
                escrow.pinned.clone(),
                escrow.chain.clone(),
                escrow.administrators.clone(),
                escrow.log.clone(),
            )
        };
        let verified = verify_escrow(
            self.organization,
            self.account,
            pinned.as_ref(),
            &chain,
            administrators.as_ref(),
            |head| log.as_ref().is_some_and(|log| log.holds(head)),
            |escrow| self.holds_escrow(escrow),
        );
        // An epoch made before a demotion this device holds is wrapped under no more: the
        // demoted administrator may hold it. Keys wait, unescrowed, for the next epoch.
        // With no organization log read this run, as what an earlier run kept of it says:
        // current only if it names one of its entries from the last demotion on.
        let kept = self
            .roots
            .lock()
            .unwrap()
            .kept
            .get(&self.organization)
            .cloned();
        let outdates = |head: &EntryHash| match (&log, &kept) {
            (Some(log), _) => log.outdates(head),
            (None, Some(kept)) => kept.lowering_place > 0 && !kept.current.contains(head),
            (None, None) => false,
        };
        let (verified, outdated) = match verified {
            Some(escrow) if outdates(&escrow.head) => (None, Some(escrow)),
            verified => (verified, None),
        };
        let mut escrow = self.escrow.lock().unwrap();
        escrow.verified = verified;
        escrow.outdated = outdated;
    }

    /// The escrow epoch keys are wrapped under: none while the organization's log this device
    /// holds is a proof path that hides a lowering (`Checked::hides_lowering`): one served
    /// honestly to a member below Edit everywhere keeps every administrator record whole, so a
    /// reduced demotion means the server hid whom it demoted, and no epoch's maker is vouched
    /// for by it.
    pub fn escrow(&self) -> Option<EscrowKey> {
        let escrow = self.escrow.lock().unwrap();
        match escrow.log.as_ref().is_some_and(|log| log.hides_lowering) {
            true => None,
            false => escrow.verified.clone(),
        }
    }

    pub fn escrow_warning(&self) -> Option<EscrowWarning> {
        let escrow = self.escrow.lock().unwrap();
        let latest = escrow.chain.iter().max_by_key(|escrow| escrow.epoch)?;
        match &escrow.verified {
            // Between a demotion and the next epoch: the latest is verified but outdated, which
            // is no fault (`settle_escrow`).
            None if escrow.outdated.as_ref() == Some(latest) => None,
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

    /// `key` wrapped under `escrow`, unless an owner-only scope covers it or it is already
    /// wrapped under it.
    pub fn escrow_wrap_under(
        &self,
        key: Uuid,
        escrow: &EscrowKey,
    ) -> Result<Option<WrappedKey>, StoreError> {
        if self.owner_only_covers(key) {
            return Ok(None);
        }
        if self.wrapped_by(key).contains(&WrappedBy::Key(escrow.id)) {
            return Ok(None);
        }
        // Sealed from this account, as to an account: only a sender holding an identity key
        // makes one, which the escrow's holders check against the organization's members.
        let plain = self.carried_plain(key)?;
        let to = Party {
            account: escrow.id.into_bytes(),
            public: RecoveryPublic(escrow.public),
        };
        Ok(Some(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(escrow.id),
            sender: self.account,
            sealed: self
                .identity
                .seal_for(self.account.into_bytes(), &to, key.as_bytes(), &plain),
        }))
    }

    /// A new escrow key, chained to the latest, with its secret sealed to this account. It is
    /// not this device's escrow until the server holds it (`adopt_escrow`): a key made
    /// meanwhile and wrapped under it would be refused by a server that does not know it.
    pub fn mint_escrow(&self, head: EntryHash) -> Result<(EscrowKey, KeySet), StoreError> {
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
            head,
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
        {
            let mut roots = self.roots.lock().unwrap();
            roots.blocks.insert(escrow.id, self.organization);
            roots.heads.insert(
                escrow.id,
                Heads {
                    block: None,
                    organization: Some(head),
                },
            );
        }
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
        match (roots.pinned.get(&block), roots.weak.get(&block)) {
            (Some(root), _) => RootPin::Pinned(*root),
            (None, Some(root)) => RootPin::Weak(*root),
            (None, None) if roots.conflicting.contains(&block) => RootPin::Conflicting,
            (None, None) => RootPin::Unknown,
        }
    }

    /// Whether this device holds anything of `block`'s access log: a root it pinned, took
    /// weakly or found conflicting, or how far it checked the log (`KeptLog`). A block it holds
    /// any of is not one it may make (`Peer::new_log`).
    pub fn holds_log(&self, block: Uuid) -> bool {
        let roots = self.roots.lock().unwrap();
        roots.pinned.contains_key(&block)
            || roots.weak.contains_key(&block)
            || roots.conflicting.contains(&block)
            || roots.kept.contains_key(&block)
    }

    /// What this device kept of `block`'s log (`KeptLog`), to write to the refs.
    pub fn kept_log(&self, block: Uuid) -> Option<crate::refs::KeptLog> {
        self.roots.lock().unwrap().kept.get(&block).cloned()
    }

    /// Lets go of `root`, made here for `block` (`Peer::new_log`), and the log kept at it, as
    /// no server took the creation: refused, made by someone else, or discarded. Only what is
    /// still that root's goes. A conflict it caused with the root keys of the block carried
    /// meanwhile is lifted, and that root stands, when they all carried the same one.
    pub fn forget_made(&self, block: Uuid, root: EntryHash) {
        let mut roots = self.roots.lock().unwrap();
        roots.unmade.insert(block, root);
        let made = roots.made.remove(&block).filter(|made| made.root == root);
        if roots.pinned.get(&block) == Some(&root) {
            roots.pinned.remove(&block);
        }
        if roots
            .kept
            .get(&block)
            .is_some_and(|kept| kept.head == root && kept.place == 0)
        {
            roots.kept.remove(&block);
        }
        let Some(made) = made else {
            return;
        };
        let [(other, weak), rest @ ..] = made.others.as_slice() else {
            return;
        };
        if rest.iter().any(|(more, _)| more != other) || !roots.conflicting.remove(&block) {
            return;
        }
        match *weak && rest.iter().all(|(_, weak)| *weak) {
            true => roots.weak.insert(block, *other),
            false => roots.pinned.insert(block, *other),
        };
    }

    /// The roots made here that no server took, from an earlier run (`forget_made`): set before
    /// the keys their creations minted are read again, which would pin them.
    pub fn set_unmade(&self, unmade: impl IntoIterator<Item = (Uuid, EntryHash)>) {
        self.roots.lock().unwrap().unmade.extend(unmade);
    }

    /// Notes `root` as made here for `block`, whose creation the outbox keeps from an earlier
    /// run, before the keys it pinned are read again (`pin_root`).
    pub fn note_made(&self, block: Uuid, root: EntryHash) {
        self.roots.lock().unwrap().made.entry(block).or_insert(Made {
            root,
            others: Vec::new(),
        });
    }

    /// The server took the creation of `block`, made here: its root is the block's.
    pub fn made_taken(&self, block: Uuid) {
        self.roots.lock().unwrap().made.remove(&block);
    }

    /// Pins the root of a block this device makes, before minting its keys, which carry it, as
    /// made here until a server takes the creation (`made_taken`) or it is let go
    /// (`forget_made`).
    pub fn pin_root(&self, block: Uuid, root: EntryHash) {
        let mut roots = self.roots.lock().unwrap();
        roots.unmade.remove(&block);
        roots.made.insert(
            block,
            Made {
                root,
                others: Vec::new(),
            },
        );
        roots.pin(block, root);
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
            heads: roots.heads.get(&key).copied().unwrap_or_default(),
        }
        .encode())
    }

    /// The log heads `key`'s epoch was made under, as this device minted it or a trusted key
    /// carried them: none it knows of when it holds neither.
    pub fn heads(&self, key: Uuid) -> Heads {
        self.roots
            .lock()
            .unwrap()
            .heads
            .get(&key)
            .copied()
            .unwrap_or_default()
    }

    /// Keeps `block`'s log as this device just checked it: the heads a key minted here for it
    /// names, and what a contact's seal of its keys is checked against.
    ///
    /// A log that does not reach the head this device kept from an earlier check is withheld
    /// (the server serves a prefix of what it served before, as after a restart): it is not
    /// taken, and epochs stay judged by what was kept. Answers what to keep anew, when it moved.
    pub fn note_log(
        &self,
        block: Uuid,
        checked: &Checked,
        entries: &[be_block_keys::Step],
    ) -> Option<crate::refs::KeptLog> {
        let mut roots = self.roots.lock().unwrap();
        if roots
            .kept
            .get(&block)
            .is_some_and(|kept| !checked.holds(&kept.head))
        {
            return None;
        }
        // On a proof path, the newest lowering this device was ever shown stands, though a later
        // path may leave out the reduced tail that held it: a made-up lowering only outdates
        // keys, which fails closed, and a real one dropped later still outdates what came before
        // it. A whole log, which keeps its head, tells exactly.
        let lowers = |step: &be_block_keys::Step| match step {
            be_block_keys::Step::Entry(entry) => entry.lowers,
            be_block_keys::Step::Reduced(reduced) => reduced.lowers,
        };
        let shown = entries.iter().rposition(lowers).unwrap_or(0);
        let seen = match checked.partial {
            true => shown.max(
                roots
                    .kept
                    .get(&block)
                    .map_or(0, |kept| kept.lowering_place as usize),
            ),
            false => shown,
        };
        let mut checked = checked.clone();
        checked.last_lowering = checked.last_lowering.max(seen);
        roots.logs.insert(block, checked.clone());
        roots.entries.insert(block, entries.to_vec());
        // Kept up to the last entry served whole: on a proof path, reduced entries after it
        // may be a tail the server made up, which no honest log reaches, and keeping its head
        // would leave every later log withheld here. A whole entry's place the server cannot
        // fake, as the next whole entry's signer names the one before it.
        let upto = match checked.partial {
            true => entries
                .iter()
                .rposition(|step| step.entry().is_some())
                .unwrap_or(0),
            false => entries.len().saturating_sub(1),
        };
        let kept = crate::refs::KeptLog {
            head: entries[upto].hash(),
            place: upto as u64,
            lowering_place: seen as u64,
            // None kept is current when the lowering lies past the last whole entry.
            current: entries
                .get(seen..=upto)
                .unwrap_or_default()
                .iter()
                .map(be_block_keys::Step::hash)
                .collect(),
            levels: {
                let mut levels: Vec<([u8; 16], Level)> = checked
                    .levels
                    .iter()
                    .filter_map(|(account, changes)| {
                        let (_, level) = changes.iter().rev().find(|(at, _)| *at <= upto)?;
                        Some((*account, *level))
                    })
                    .collect();
                levels.sort();
                levels
            },
            vouched: roots
                .kept
                .get(&block)
                .map(|kept| kept.vouched.clone())
                .unwrap_or_default(),
        };
        match roots.kept.insert(block, kept.clone()) == Some(kept.clone()) {
            true => None,
            false => Some(kept),
        }
    }

    /// What to make of `checked`, a log the server served for `block` (or the organization):
    /// taken when it reaches what this device kept of it; when it stops short, stale if this
    /// run already read further (two reads raced, and the older answer came second), so the
    /// newer is used, or else withheld, and nothing is taken from it.
    pub fn served(&self, block: Uuid, checked: &Checked) -> Served {
        let roots = self.roots.lock().unwrap();
        let Some(kept) = roots.kept.get(&block) else {
            return Served::Take;
        };
        if checked.holds(&kept.head) {
            return Served::Take;
        }
        match (roots.logs.get(&block), roots.entries.get(&block)) {
            (Some(newer), Some(entries)) if newer.holds(&kept.head) => {
                Served::Stale(Box::new((newer.clone(), entries.clone())))
            }
            _ => Served::Withheld,
        }
    }

    /// The scopes whose keys this device holds trusted, with their roots pinned, whose logs it
    /// has kept nothing of: `Peer::refresh_keys` reads each once, so a device later offline
    /// judges their epochs by what it kept rather than refusing to write.
    pub fn unkept_scopes(&self) -> Vec<Uuid> {
        let scopes: Vec<Uuid> = self
            .keys_of_kind(KeyKind::Scope)
            .into_iter()
            .filter(|row| self.trusted(row.id))
            .map(|row| row.block)
            .collect();
        let roots = self.roots.lock().unwrap();
        let mut found: Vec<Uuid> = scopes
            .into_iter()
            .filter(|block| roots.pinned.contains_key(block) && !roots.kept.contains_key(block))
            .collect();
        found.sort();
        found.dedup();
        found
    }

    /// How far this device checked logs in an earlier run (`note_log`), from the refs.
    pub fn set_kept_logs(&self, kept: impl IntoIterator<Item = (Uuid, crate::refs::KeptLog)>) {
        self.roots.lock().unwrap().kept.extend(kept);
    }

    /// Keeps `block`, made here, as checked at its root: its log can be read from the server
    /// only once the server holds it, and nothing in it lowered anyone.
    pub fn keep_made(&self, block: Uuid, root: EntryHash) -> crate::refs::KeptLog {
        let kept = crate::refs::KeptLog {
            head: root,
            place: 0,
            lowering_place: 0,
            current: vec![root],
            levels: vec![(self.account.into_bytes(), Level::Manage)],
            vouched: Vec::new(),
        };
        self.roots
            .lock()
            .unwrap()
            .kept
            .entry(block)
            .or_insert(kept.clone());
        kept
    }

    /// The heads a key of `block` minted now is made under: the newest of `block`'s log and of
    /// the organization's this device checked, else the head an earlier run kept, or the root it
    /// pinned for a block it has not checked the log of (as on making it).
    fn current_heads(&self, block: Uuid) -> Heads {
        let block_head = {
            let roots = self.roots.lock().unwrap();
            roots
                .logs
                .get(&block)
                .map(|checked| checked.head)
                .or_else(|| roots.kept.get(&block).map(|kept| kept.head))
                .or_else(|| roots.pinned.get(&block).copied())
        };
        let organization = self
            .escrow
            .lock()
            .unwrap()
            .log
            .as_ref()
            .map(|checked| checked.head)
            .or_else(|| {
                let roots = self.roots.lock().unwrap();
                roots.kept.get(&self.organization).map(|kept| kept.head)
            });
        Heads {
            block: block_head,
            organization,
        }
    }

    /// Whether a key `sender`, a contact, sealed to this account, carried as `carried`, may be
    /// taken (guides/blocks/keys.md, "Epochs name their heads"). An escrow key only from an
    /// administrator the organization's log names at its head as this device checked it: never as
    /// of a head the sender names, since a demoted administrator would name one from before. Any
    /// other key from a sender the block's log gives Edit or above, or the organization's log has
    /// administer it, as of the head the key names, or as of the log's head here when this
    /// device's log does not reach that head (or the key names none). A sender the block's log
    /// names, but not as of that head, held nothing then. A key from one it never names is taken
    /// only once this device found, as that key arrived, that its sender held Edit or above on
    /// a block the signed parent records place the block under (`vouch`, guides/blocks/keys.md,
    /// "Parent records"); any key of a block whose log this device never checked is taken (its
    /// first, which carries the root).
    ///
    /// A data key is taken from a contact only as an administrator (the settings' data keys,
    /// which key work has them seal to each other): no one else seals one to an account, and
    /// its epoch names its scope's log, which its own block's is not.
    /// Notes that the wrap of `key`, a key of `block`, that `sender` sealed as `sealed` may be
    /// taken though the block's log never names `sender`: it held Edit or above on a block the
    /// signed parent records place the block under as the wrap arrived, as `Peer::held_above`
    /// found (`sender_may`), across restarts too. That wrap alone: any other wrap of the key,
    /// from that sender or another, is judged as it arrives. Answers what to keep when this
    /// changed what was kept.
    pub fn vouch(
        &self,
        block: Uuid,
        key: Uuid,
        sender: Uuid,
        sealed: &[u8],
    ) -> Option<crate::refs::KeptLog> {
        let vouched = (key.into_bytes(), sender.into_bytes(), sealed_digest(sealed));
        let mut roots = self.roots.lock().unwrap();
        let kept = roots.kept.get_mut(&block)?;
        if kept.vouched.contains(&vouched) {
            return None;
        }
        kept.vouched.push(vouched);
        Some(kept.clone())
    }

    /// Whether `block`'s log, as this device checked or kept it, names `account` at all.
    pub fn names(&self, block: Uuid, account: Uuid) -> bool {
        let roots = self.roots.lock().unwrap();
        let account = account.into_bytes();
        match roots.logs.get(&block) {
            Some(log) => log.levels.contains_key(&account),
            None => roots
                .kept
                .get(&block)
                .is_some_and(|kept| kept.levels.iter().any(|(named, _)| *named == account)),
        }
    }

    fn sender_may(
        &self,
        sender: Uuid,
        key: Uuid,
        sealed: &[u8],
        carried: &CarriedKey,
        kind: KeyKind,
    ) -> bool {
        if sender == self.account {
            return true;
        }
        let account = sender.into_bytes();
        let kept_administers = {
            let roots = self.roots.lock().unwrap();
            roots.kept.get(&self.organization).is_some_and(|kept| {
                kept.levels
                    .iter()
                    .any(|(named, level)| *named == account && *level == Level::Manage)
            })
        };
        let administered = {
            let escrow = self.escrow.lock().unwrap();
            let escrow_key = carried.block == self.organization.into_bytes();
            match escrow.log.as_ref() {
                Some(log) => match carried.heads.organization.filter(|head| log.holds(head)) {
                    Some(head) if !escrow_key => {
                        log.level_as_of(account, &head) == Some(Level::Manage)
                    }
                    _ => log.administrators.contains_key(&account),
                },
                // No organization log read this run: as what an earlier run kept of it says.
                None => kept_administers,
            }
        };
        // An escrow key travels as the organization's: only an administrator hands it on.
        if administered || carried.block == self.organization.into_bytes() || kind != KeyKind::Scope
        {
            return administered;
        }
        let block = Uuid::from_bytes(carried.block);
        let roots = self.roots.lock().unwrap();
        // A block checked in an earlier run but whose log this run has not read (a restart,
        // a refused or short read, offline): judged by the levels kept at the head then, so a
        // sender revoked by then is refused whatever head the key names.
        let wrap = (key.into_bytes(), account, sealed_digest(sealed));
        let vouched = roots
            .kept
            .get(&block)
            .is_some_and(|kept| kept.vouched.contains(&wrap));
        let Some(log) = roots.logs.get(&block) else {
            return roots.kept.get(&block).is_none_or(|kept| {
                match kept.levels.iter().find(|(named, _)| *named == account) {
                    Some((_, level)) => *level >= Level::Edit,
                    None => vouched,
                }
            });
        };
        if !log.levels.contains_key(&account) {
            return vouched;
        }
        let head = carried
            .heads
            .block
            .filter(|head| log.holds(head))
            .unwrap_or(log.head);
        log.level_as_of(account, &head)
            .is_some_and(|level| level >= Level::Edit)
    }

    /// Whether `block`'s log, as this device last checked it, outdates `key`'s epoch: the key
    /// names a head of it before an entry that lowered someone's access, who may hold it
    /// (`Checked::outdates`). A device writes and wraps under such a key no more: it rotates.
    ///
    /// A key naming no head, or one the log does not reach, counts as named at the root: a
    /// device would otherwise write under a key someone made up (wrapped under an epoch they
    /// held, so no seal names its sender) for as long as it names a head no log holds. An
    /// honest head newer than this device's log costs a rotation (`Peer::outdated` reads the
    /// log again first). And a key counts as outdated when the epoch it was opened through is,
    /// whoever's head it names (`Roots::through`), or when the organization's log demoted an
    /// administrator after the head of it the key names (`demoted_since`).
    pub fn outdated(&self, key: Uuid, block: Uuid) -> bool {
        self.judged(key, block, true)
    }

    /// `outdated`, but a scope this device neither read nor kept the log of counts for nothing:
    /// what an edit already made is judged by before it is sent, set aside only on evidence.
    pub fn shown_outdated(&self, key: Uuid, block: Uuid) -> bool {
        self.judged(key, block, false)
    }

    fn judged(&self, key: Uuid, block: Uuid, unknown_outdates: bool) -> bool {
        let roots = self.roots.lock().unwrap();
        let Some(path) = roots.path(key, block) else {
            return true;
        };
        // A key opened through no other (made here, or sealed to this account, whose sender
        // was checked as it was taken) is judged only by logs this device holds or kept.
        let opened_through = unknown_outdates && path.len() > 1;
        path.into_iter().any(|(key, block)| {
            let heads = roots.heads.get(&key).copied().unwrap_or_default();
            if self.demoted_since(&roots, key, heads.organization) {
                return true;
            }
            let head = heads.block;
            if let Some(log) = roots.logs.get(&block) {
                return match head.filter(|head| log.holds(head)) {
                    Some(head) => log.outdates(&head),
                    None => log.last_lowering > 0,
                };
            }
            // No log read in this run (offline, refused, or withheld): judged by how far an
            // earlier run checked it, whose entries from its last lowering through its head
            // are known current; a head newer than those is outdated until a log reaches it.
            if let Some(kept) = roots.kept.get(&block) {
                return kept.lowering_place > 0
                    && !head.is_some_and(|head| kept.current.contains(&head));
            }
            opened_through
        })
    }

    /// Whether a demotion in the organization's log outdates `key`, of `block`'s scope, or any
    /// key it was opened through (`demoted_since`): what key work rotates at once
    /// (`Peer::rotate_demoted`).
    pub fn demoted(&self, key: Uuid, block: Uuid) -> bool {
        let roots = self.roots.lock().unwrap();
        roots.path(key, block).is_some_and(|path| {
            path.into_iter().any(|(key, _)| {
                let head = roots.heads.get(&key).and_then(|heads| heads.organization);
                self.demoted_since(&roots, key, head)
            })
        })
    }

    /// Whether the organization's log, as this device last checked it (or, with none read this
    /// run, kept it), outdates the epoch of `key`, which names `head` of it: an administrator
    /// was demoted or signed out after it, and may hold the epoch through escrow. So a demotion
    /// makes every scope under the escrow due a rotation at its next write, as a revocation does
    /// its block's. A head the log does not reach counts as its root, as for a block's log. Never a
    /// key an owner-only scope covers, which no escrow holds; never in an organization this device
    /// holds no log of (the personal one has none).
    fn demoted_since(&self, roots: &Roots, key: Uuid, head: Option<EntryHash>) -> bool {
        if self.owner_only_covers(key) {
            return false;
        }
        if let Some(log) = roots.logs.get(&self.organization) {
            return match head.filter(|head| log.holds(head)) {
                Some(head) => log.outdates(&head),
                None => log.last_lowering > 0,
            };
        }
        roots.kept.get(&self.organization).is_some_and(|kept| {
            kept.lowering_place > 0 && !head.is_some_and(|head| kept.current.contains(&head))
        })
    }

    /// The scopes whose logs `outdated` judges `key`, of `block`'s scope, by, that this device
    /// has not checked, or whose log does not reach the head the key there names: what
    /// `Peer::outdated` reads first. The organization's among them, by its id, when it has a log
    /// and a key on the path names a head of it this device does not hold.
    pub fn logs_to_read(&self, key: Uuid, block: Uuid) -> Vec<Uuid> {
        let organization_root = self.organization_root();
        let roots = self.roots.lock().unwrap();
        let path = roots.path(key, block).unwrap_or_default();
        let organization = organization_root.is_some()
            && path.iter().any(|(key, _)| {
                let head = roots.heads.get(key).and_then(|heads| heads.organization);
                !self.owner_only_covers(*key)
                    && match roots.logs.get(&self.organization) {
                        None => true,
                        Some(log) => head.is_some_and(|head| !log.holds(&head)),
                    }
            });
        let mut found: Vec<Uuid> = path
            .into_iter()
            .filter(|(key, block)| match roots.logs.get(block) {
                None => true,
                Some(log) => roots
                    .heads
                    .get(key)
                    .and_then(|heads| heads.block)
                    .is_some_and(|head| !log.holds(&head)),
            })
            .map(|(_, block)| block)
            .chain(organization.then_some(self.organization))
            .collect();
        found.sort();
        found.dedup();
        found
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
        if vouched {
            roots.heads.entry(row.id).or_insert(carried.heads);
        }
        Some(Opened {
            bytes: carried.key,
            block: row.block,
            // Bound inside the seal: an escrow key travels as the organization's, which no
            // block is (its settings block has an id of its own).
            escrow: carried.block == self.organization.into_bytes(),
            through_server: false,
            under_owner_only: false,
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
                let vouched = wraps.iter().find_map(|wrap| self.unwrap(wrap, row, true));
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
                if let Some(bytes) = wraps.iter().find_map(|wrap| self.unwrap(wrap, row, false)) {
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
        if changed > 0 {
            self.told_opened();
        }
        changed
    }

    /// Keeps an opened key: an escrow's secret apart from the content keys.
    fn hold(&self, key: Uuid, opened: Opened) {
        if opened.through_server {
            self.server_held.lock().unwrap().insert(key);
        }
        if opened.under_owner_only {
            self.under_owner_only.lock().unwrap().insert(key);
        }
        match opened.escrow {
            true => {
                self.escrow_secrets
                    .lock()
                    .unwrap()
                    .insert(key, opened.bytes);
            }
            false => self
                .ring
                .insert(key, ContentKey::from_bytes(opened.bytes), opened.block),
        }
    }

    /// Holds a key opened from `wrap` for `Peer::settle_escrowed`, when what it opened to is a
    /// key of the block `row` names.
    fn hold_escrowed(&self, wrap: &WrappedKey, row: KeyRow, plain: &[u8], escrow: bool) {
        let Ok(carried) = CarriedKey::decode(plain) else {
            return;
        };
        if carried.block != row.block.into_bytes() {
            return;
        }
        self.escrowed.lock().unwrap().insert(
            wrap.key,
            (
                Escrowed {
                    key: wrap.key,
                    block: row.block,
                    sender: wrap.sender,
                    root: carried.root,
                    heads: carried.heads,
                    escrow,
                },
                carried.key,
            ),
        );
    }

    fn unwrap(&self, wrap: &WrappedKey, row: Option<KeyRow>, vouched: bool) -> Option<Opened> {
        let block = row.map(|row| row.block);
        let bytes = match wrap.wrapped_by {
            WrappedBy::Account(account) if account == self.account => {
                if !self.accepts(wrap.sender, block, vouched) {
                    return None;
                }
                let Some(public) = self.sender(wrap.sender) else {
                    // A member the organization's log names who is not a contact: opened with
                    // the keys the log names, then held for `Peer::settle_escrowed`, as an
                    // escrow wrap is.
                    if vouched {
                        return None;
                    }
                    let public = self
                        .escrow
                        .lock()
                        .unwrap()
                        .members
                        .get(&wrap.sender)?
                        .public;
                    let from = Party {
                        account: wrap.sender.into_bytes(),
                        public,
                    };
                    let plain = self
                        .identity
                        .open_from(
                            &from,
                            self.account.into_bytes(),
                            wrap.key.as_bytes(),
                            &wrap.sealed,
                        )
                        .ok()?;
                    self.hold_escrowed(wrap, row?, &plain, false);
                    return None;
                };
                let sender = Party {
                    account: wrap.sender.into_bytes(),
                    public,
                };
                let plain = self
                    .identity
                    .open_from(
                        &sender,
                        self.account.into_bytes(),
                        wrap.key.as_bytes(),
                        &wrap.sealed,
                    )
                    .ok()?;
                if self.contact(wrap.sender).is_some()
                    && !self.sender_may(
                        wrap.sender,
                        wrap.key,
                        &wrap.sealed,
                        &CarriedKey::decode(&plain).ok()?,
                        row.map_or(KeyKind::Data, |row| row.kind),
                    )
                {
                    return None;
                }
                plain
            }
            WrappedBy::Account(_) => return None,
            // Opened only from a member the organization's log names, with the keys it names,
            // then held until `Peer::settle_escrowed` finds them granted in the block's log.
            WrappedBy::Key(by) if self.escrow_secret(by).is_some() => {
                if vouched || self.escrowed.lock().unwrap().contains_key(&wrap.key) {
                    return None;
                }
                let row = row?;
                let sender = *self.escrow.lock().unwrap().members.get(&wrap.sender)?;
                let from = Party {
                    account: wrap.sender.into_bytes(),
                    public: sender.public,
                };
                let plain = self
                    .escrow_secret(by)?
                    .open_from(&from, by.into_bytes(), wrap.key.as_bytes(), &wrap.sealed)
                    .ok()?;
                self.hold_escrowed(wrap, row, &plain, true);
                return None;
            }
            WrappedBy::Key(by) => {
                if be_block_store::sealed_key(&wrap.sealed).ok()? != by || !self.ring.contains(by) {
                    return None;
                }
                // A key the server holds vouches for nothing: what it wraps opens untrusted, and
                // is server-held too.
                let through_server = self.server_held(by);
                if vouched && (through_server || !self.trusted(by)) {
                    return None;
                }
                let bytes = self.ring.open(&wrap.sealed).ok()?;
                let mut opened = self.opened(row, &bytes, vouched)?;
                if vouched {
                    let mut roots = self.roots.lock().unwrap();
                    let through = roots.through.entry(wrap.key).or_default();
                    if !through.contains(&by) {
                        through.push(by);
                    }
                }
                opened.through_server = through_server;
                opened.under_owner_only = self.owner_only_covers(by);
                return Some(opened);
            }
        };
        self.opened(row, &bytes, vouched)
    }

    /// Whether the server holds `key`, or may: a key of the organization's settings block,
    /// which is sealed to the server for it to read the policies, or one opened only through
    /// such a key. It opens, but vouches for no key wrapped under it, and seals only blocks in
    /// the settings (`Peer::sealing_key`).
    pub fn server_held(&self, key: Uuid) -> bool {
        self.block_of(key) == Some(be_block::organization_settings(self.organization))
            || self.server_held.lock().unwrap().contains(&key)
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
        let heads = self.current_heads(block);
        self.ring.insert(row.id, ContentKey::random(), block);
        self.trusted.lock().unwrap().insert(row.id);
        let mut roots = self.roots.lock().unwrap();
        roots.blocks.insert(row.id, block);
        roots.heads.insert(row.id, heads);
        row
    }

    pub fn mint_data(&self, block: Uuid, under: Uuid) -> Result<(Uuid, KeySet), StoreError> {
        self.trusted_plain(under)?;
        let row = self.mint(block, KeyKind::Data, false);
        // A data key's epoch is its scope's: it names the heads of the log of the scope it is
        // wrapped under, which grants who reads it.
        if let Some(scope) = self.block_of(under) {
            let heads = self.current_heads(scope);
            self.roots.lock().unwrap().heads.insert(row.id, heads);
        }
        if self.owner_only_covers(under) {
            self.under_owner_only.lock().unwrap().insert(row.id);
        }
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

    /// Whether an owner-only block's scope key covers `key`: it is one (told by the key's own
    /// id, which every seal of it binds, so the server cannot say otherwise), or this device
    /// minted or opened `key` under one, whatever the server's rows say. Such a key is its
    /// author's alone, never escrowed, so never sealed to anyone else either.
    pub fn owner_only_covers(&self, key: Uuid) -> bool {
        be_block_protocol::owner_only_key(key)
            || self.under_owner_only.lock().unwrap().contains(&key)
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
        keys.into_iter().filter(|key| !self.has(*key)).collect()
    }
}

/// What a vouch names a wrap's sealed bytes by (`KeyBook::vouch`).
fn sealed_digest(sealed: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::new()
        .chain_update(b"be3.vouched.v1")
        .chain_update(sealed)
        .finalize()
        .into()
}
