use rusqlite::Connection;

use crate::ServerError;

pub fn initialize(connection: &Connection) -> Result<(), ServerError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    be_block_sqlite_schema::ensure(connection, SCHEMA)?;
    Ok(())
}

const SCHEMA: &str = "
    CREATE TABLE accounts (
        id              TEXT PRIMARY KEY,
        -- The verified email, if the account has one: an email identity's, or an authoritative
        -- provider's. None for an account made through a provider that gave no verified email.
        email           TEXT UNIQUE,
        display_name    TEXT NOT NULL
    );

    CREATE TABLE sessions (
        token_hash      TEXT PRIMARY KEY,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        kind            TEXT NOT NULL DEFAULT 'legacy',
        public_id       TEXT NOT NULL DEFAULT '',
        created         INTEGER NOT NULL DEFAULT 0,
        expires         INTEGER NOT NULL DEFAULT 0,
        proved          INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE identities (
        id              TEXT PRIMARY KEY,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        kind            TEXT NOT NULL,
        subject         TEXT NOT NULL,
        created         INTEGER NOT NULL,
        -- For a provider (`oidc`), the verified email it last gave, if it is authoritative for
        -- email.
        email           TEXT NOT NULL DEFAULT '',
        -- For a passkey, the key and signature counter webauthn-rs checks it with (JSON).
        credential      TEXT NOT NULL DEFAULT '',
        UNIQUE (kind, subject)
    );
    CREATE INDEX identities_account ON identities(account_id);

    CREATE TABLE sign_in_codes (
        attempt_hash    TEXT PRIMARY KEY,
        code_hash       TEXT NOT NULL,
        purpose         TEXT NOT NULL,
        account_id      TEXT NOT NULL DEFAULT '',
        email           TEXT NOT NULL,
        display_name    TEXT NOT NULL DEFAULT '',
        invite_hash     TEXT NOT NULL DEFAULT '',
        device_code     TEXT NOT NULL DEFAULT '',
        tries           INTEGER NOT NULL DEFAULT 0,
        expires         INTEGER NOT NULL,
        created         INTEGER NOT NULL,
        address         TEXT NOT NULL,
        network         TEXT NOT NULL,
        remembered      INTEGER NOT NULL DEFAULT 0,
        -- For a code that links a provider: its identity's subject and the email it gave.
        identity        TEXT NOT NULL DEFAULT '',
        identity_email  TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX sign_in_codes_address ON sign_in_codes(address, created);
    CREATE INDEX sign_in_codes_network ON sign_in_codes(network, created);
    CREATE INDEX sign_in_codes_account ON sign_in_codes(account_id, created);

    CREATE TABLE oidc_states (
        state_hash      TEXT PRIMARY KEY,
        provider        TEXT NOT NULL,
        verifier        TEXT NOT NULL,
        nonce           TEXT NOT NULL,
        attempt_hash    TEXT NOT NULL,
        device_code     TEXT NOT NULL DEFAULT '',
        intent          TEXT NOT NULL,
        link_account    TEXT NOT NULL DEFAULT '',
        address         TEXT NOT NULL,
        expires         INTEGER NOT NULL
    );

    CREATE TABLE oidc_pending (
        attempt_hash    TEXT PRIMARY KEY,
        provider        TEXT NOT NULL,
        subject         TEXT NOT NULL,
        email           TEXT NOT NULL DEFAULT '',
        display_name    TEXT NOT NULL DEFAULT '',
        device_code     TEXT NOT NULL DEFAULT '',
        expires         INTEGER NOT NULL
    );

    CREATE TABLE login_links (
        token_hash TEXT PRIMARY KEY,
        account_id TEXT NOT NULL DEFAULT '',
        email      TEXT NOT NULL DEFAULT '',
        expires    INTEGER NOT NULL
    );

    -- An account's mailbox (`store/mailbox.rs`): each message kept until a device of the
    -- account deletes it. `sender_id` is '' for the server's own notices; `letter` is the
    -- encoded `be_block_protocol::Letter`, and `kind` its case (`notice`, `accepted`,
    -- `key-change`), which the hold and the bounds read; `created` is in milliseconds.
    CREATE TABLE mailbox (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        sender_id       TEXT NOT NULL DEFAULT '',
        kind            TEXT NOT NULL,
        letter          BLOB NOT NULL,
        created         INTEGER NOT NULL
    );
    CREATE INDEX mailbox_account ON mailbox(account_id, id);
    CREATE INDEX mailbox_sender ON mailbox(sender_id);

    -- Ways to sign in added to or removed from an account, and its email changing (`kind`:
    -- `added`, `removed`, `email`), kept `IDENTITY_HISTORY` for the portal's settings page and
    -- to cap how many are added a day.
    CREATE TABLE identity_changes (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        kind            TEXT NOT NULL,
        label           TEXT NOT NULL,
        created         INTEGER NOT NULL
    );
    CREATE INDEX identity_changes_account ON identity_changes(account_id, created);

    CREATE TABLE account_mail (
        token_hash TEXT PRIMARY KEY,
        account_id TEXT NOT NULL DEFAULT '',
        email TEXT NOT NULL,
        expires INTEGER NOT NULL,
        address TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX account_mail_email ON account_mail(email);
    CREATE INDEX account_mail_address ON account_mail(address);

    CREATE TABLE organizations (
        id              TEXT PRIMARY KEY,
        name            TEXT NOT NULL,
        owner_id        TEXT REFERENCES accounts(id),
        kind            TEXT NOT NULL DEFAULT 'company',
        change_sequence INTEGER NOT NULL DEFAULT 0
    );

    -- A company organization's access log (guides/blocks/keys.md), in order,
    -- as access_log keeps a block's.
    CREATE TABLE organization_log (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        position        INTEGER NOT NULL,
        entry           BLOB NOT NULL,
        PRIMARY KEY (organization_id, position)
    );

    -- Every account the organization's log names keys for (its signers,
    -- grantees and administrators): a new recovery key retires each in it.
    CREATE TABLE organization_log_names (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        account_id      TEXT NOT NULL,
        PRIMARY KEY (organization_id, account_id)
    );

    CREATE INDEX organization_log_names_account ON organization_log_names (account_id);

    CREATE TABLE memberships (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        role            TEXT NOT NULL,
        PRIMARY KEY (organization_id, account_id)
    );

    CREATE TABLE links (
        id              TEXT PRIMARY KEY,
        owner_id        TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        card            BLOB NOT NULL,
        grant_organization TEXT NOT NULL DEFAULT '',
        grant_block     TEXT NOT NULL DEFAULT '',
        grant_access    TEXT NOT NULL DEFAULT '',
        grant_key       TEXT NOT NULL DEFAULT '',
        grant_sealed    BLOB NOT NULL DEFAULT x'',
        invite_organization TEXT NOT NULL DEFAULT '',
        invite_escrow   BLOB NOT NULL DEFAULT x'',
        -- An invitation's key, derived from its code, which its record in the organization's
        -- log names and which signs its accepter in.
        invite_key      BLOB NOT NULL DEFAULT x'',
        -- An invitation's times, in seconds since the epoch: made, expiring, first read, and
        -- accepted or revoked (0 for not yet).
        invite_created  INTEGER NOT NULL DEFAULT 0,
        invite_expires  INTEGER NOT NULL DEFAULT 0,
        invite_opened   INTEGER NOT NULL DEFAULT 0,
        invite_ended    INTEGER NOT NULL DEFAULT 0,
        invite_revoked  INTEGER NOT NULL DEFAULT 0,
        -- Its inviter's device finished it: made the accepter a contact
        -- (`delete-link` on an accepted one);
        -- the row stays for the organization's invite list until its retention passes.
        invite_finished INTEGER NOT NULL DEFAULT 0,
        accepter_id     TEXT NOT NULL DEFAULT ''
    );

    CREATE INDEX links_owner
        ON links (owner_id);

    CREATE INDEX links_invitations
        ON links (invite_organization, invite_created);

    CREATE TABLE contacts (
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        contact_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        PRIMARY KEY (account_id, contact_id)
    );

    CREATE TABLE blocks (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        id              TEXT NOT NULL,
        content_type    TEXT NOT NULL,
        author          TEXT NOT NULL,
        parent_kind     INTEGER NOT NULL,
        parent_id       TEXT,
        head            TEXT,
        metadata        BLOB NOT NULL DEFAULT x'',
        version         INTEGER NOT NULL DEFAULT 0,
        key_id          TEXT NOT NULL DEFAULT '',
        payer           TEXT NOT NULL,
        owner_only      INTEGER NOT NULL DEFAULT 0,
        changed         INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, id)
    );

    -- Each block's access log (guides/blocks/keys.md), in order: entries as
    -- be_block_keys::Entry::encode wrote them, each checked as devices check it.
    CREATE TABLE access_log (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        position        INTEGER NOT NULL,
        entry           BLOB NOT NULL,
        PRIMARY KEY (organization_id, block_id, position),
        FOREIGN KEY (organization_id, block_id)
            REFERENCES blocks(organization_id, id) ON DELETE CASCADE
    );

    -- Every account a block's log names keys for, as organization_log_names.
    CREATE TABLE access_log_names (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id, account_id),
        FOREIGN KEY (organization_id, block_id)
            REFERENCES blocks(organization_id, id) ON DELETE CASCADE
    );

    CREATE INDEX access_log_names_account ON access_log_names (account_id);

    -- Each record in a block's log whose signer's right comes from a block above
    -- (be_block_keys::Authority::Inherited): whose records that block's log serves whole to a
    -- reader of a block below it who may not view it, so the reader checks the signer there.
    -- Only the chain's last block: the blocks between are proven by their parent records,
    -- which every path serves whole.
    CREATE TABLE access_log_authorities (
        organization_id TEXT NOT NULL,
        above_id        TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        signer          TEXT NOT NULL,
        PRIMARY KEY (organization_id, above_id, block_id, signer),
        FOREIGN KEY (organization_id, block_id)
            REFERENCES blocks(organization_id, id) ON DELETE CASCADE
    );

    -- Grants a block's log names that the server revoked on its own (remove-contact,
    -- delete-agent), until a device's signed revocation goes in the log.
    CREATE TABLE unsigned_revocations (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id, account_id),
        FOREIGN KEY (organization_id, block_id)
            REFERENCES blocks(organization_id, id) ON DELETE CASCADE
    );

    CREATE INDEX blocks_changed
        ON blocks (organization_id, changed, id);

    CREATE TABLE block_departures (
        organization_id TEXT NOT NULL,
        entry           TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        changed         INTEGER NOT NULL,
        time            INTEGER NOT NULL,
        owner           TEXT,
        PRIMARY KEY (organization_id, entry, block_id)
    );

    CREATE INDEX block_departures_changed
        ON block_departures (organization_id, entry, changed, block_id);

    CREATE INDEX block_departures_time
        ON block_departures (organization_id, time);

    CREATE TABLE block_edges (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        reference_id    TEXT NOT NULL,
        position        INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, block_id, reference_id)
    );

    CREATE TABLE block_access (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL,
        access          TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id, account_id)
    );

    CREATE TABLE opened_kinds (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id)
    );

    CREATE TABLE block_commits (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        commit_id       TEXT NOT NULL,
        parents         TEXT NOT NULL DEFAULT '',
        time            INTEGER NOT NULL,
        pinned          INTEGER NOT NULL CHECK (pinned IN (0, 1)),
        received        INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, block_id, commit_id)
    );

    CREATE TABLE commit_objects (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        commit_id       TEXT NOT NULL,
        position        INTEGER NOT NULL,
        hash            TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id, commit_id, position)
    );

    CREATE TABLE block_objects (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        hash            TEXT NOT NULL,
        -- When an `until-committed` hold was last made, which ends `upload_grace` after it;
        -- NULL for a lasting one.
        held_since      INTEGER,
        PRIMARY KEY (organization_id, block_id, hash)
    );

    CREATE TABLE object_refs (
        hash            TEXT PRIMARY KEY,
        count           INTEGER NOT NULL CHECK (count >= 0)
    );

    CREATE TABLE recovery_keys (
        account_id      TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
        public          BLOB NOT NULL,
        signing         BLOB NOT NULL DEFAULT x''
    );

    CREATE TABLE keys (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        id              TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        kind            TEXT NOT NULL,
        epoch           INTEGER NOT NULL,
        created         INTEGER NOT NULL,
        public          BLOB NOT NULL DEFAULT x'',
        signing         BLOB NOT NULL DEFAULT x'',
        -- An escrow key's maker, whose identity key made `signature`, and the
        -- head of the organization's log it was made under.
        maker           TEXT NOT NULL DEFAULT '',
        head            BLOB NOT NULL DEFAULT x'',
        signature       BLOB NOT NULL DEFAULT x'',
        PRIMARY KEY (organization_id, id)
    );

    CREATE TABLE rotations (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        block_id        TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id)
    );

    CREATE INDEX keys_block
        ON keys (organization_id, block_id);

    CREATE TABLE wrapped_keys (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        key_id          TEXT NOT NULL,
        wrapped_by      TEXT NOT NULL,
        sender          TEXT NOT NULL,
        sealed          BLOB NOT NULL,
        PRIMARY KEY (organization_id, key_id, wrapped_by)
    );

    CREATE INDEX wrapped_keys_by
        ON wrapped_keys (wrapped_by);

    CREATE INDEX block_edges_reference
        ON block_edges (organization_id, reference_id);

    CREATE TABLE agents (
        account_id      TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        created_by      TEXT NOT NULL,
        bound_to        TEXT
    );

    CREATE INDEX commit_objects_hash
        ON commit_objects (organization_id, hash);

    CREATE INDEX block_objects_hash
        ON block_objects (organization_id, hash);

    CREATE INDEX block_objects_held_since
        ON block_objects (held_since) WHERE held_since IS NOT NULL;

    CREATE INDEX organizations_owner
        ON organizations (owner_id);
    CREATE INDEX links_invite_organization
        ON links (invite_organization);
    CREATE TABLE server_account (
        id              INTEGER PRIMARY KEY CHECK (id = 1),
        account_id      TEXT NOT NULL REFERENCES accounts(id),
        secret          BLOB NOT NULL
    );

    CREATE TABLE invite_codes (
        id              TEXT PRIMARY KEY,
        code_hash       TEXT NOT NULL UNIQUE,
        created         INTEGER NOT NULL,
        expires         INTEGER NOT NULL DEFAULT 0,
        uses            INTEGER,
        used            INTEGER NOT NULL DEFAULT 0
    );

    CREATE INDEX agents_created_by ON agents (created_by);
    CREATE INDEX agents_bound_to ON agents (organization_id, bound_to);

    CREATE TABLE object_sizes (
        hash            TEXT PRIMARY KEY,
        bytes           INTEGER NOT NULL,
        confirmed       INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE object_uploads (
        organization_id TEXT NOT NULL,
        account_id      TEXT NOT NULL,
        hash            TEXT NOT NULL,
        payer           TEXT NOT NULL,
        uploaded        INTEGER NOT NULL,
        proven          INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, account_id, hash)
    );

    CREATE INDEX object_uploads_payer
        ON object_uploads (payer, hash);

    CREATE INDEX object_uploads_object
        ON object_uploads (hash);

    CREATE INDEX object_uploads_uploaded
        ON object_uploads (uploaded);

    CREATE TABLE block_sizes (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        size            INTEGER NOT NULL DEFAULT 0,
        tree_size       INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, block_id)
    );

    CREATE TABLE account_objects (
        account_id      TEXT NOT NULL,
        hash            TEXT NOT NULL,
        count           INTEGER NOT NULL CHECK (count > 0),
        PRIMARY KEY (account_id, hash)
    );

    CREATE TABLE account_usage (
        account_id      TEXT PRIMARY KEY,
        held            INTEGER NOT NULL DEFAULT 0,
        pending         INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE organization_usage (
        organization_id TEXT PRIMARY KEY,
        bytes           INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE block_journals (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        tag             BLOB NOT NULL,
        base_commit     TEXT,
        base_sequence   INTEGER NOT NULL,
        base_state      BLOB NOT NULL,
        sequence        INTEGER NOT NULL,
        barrier         INTEGER NOT NULL,
        lost            INTEGER NOT NULL DEFAULT 0,
        run             INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, block_id)
    );

    CREATE TABLE journal_entries (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        sequence        INTEGER NOT NULL,
        origin          INTEGER NOT NULL,
        counter         INTEGER NOT NULL,
        base            INTEGER NOT NULL,
        floor           INTEGER NOT NULL,
        received        INTEGER,
        payload         BLOB NOT NULL,
        PRIMARY KEY (organization_id, block_id, sequence)
    );

    CREATE UNIQUE INDEX journal_entries_by_id
        ON journal_entries (organization_id, block_id, origin, counter);

    CREATE TABLE block_log_heads (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        size            INTEGER NOT NULL,
        root            BLOB NOT NULL,
        bytes           INTEGER NOT NULL,
        PRIMARY KEY (organization_id, block_id)
    );

    CREATE TABLE block_logs (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        seq             INTEGER NOT NULL,
        id              BLOB NOT NULL,
        author          TEXT NOT NULL,
        arrival         INTEGER NOT NULL,
        payload_hash    BLOB NOT NULL,
        signature       BLOB NOT NULL,
        payload         BLOB,
        PRIMARY KEY (organization_id, block_id, seq)
    );

    CREATE UNIQUE INDEX block_logs_by_id
        ON block_logs (organization_id, block_id, id);

    CREATE TABLE block_log_nodes (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        level           INTEGER NOT NULL,
        position        INTEGER NOT NULL,
        hash            BLOB NOT NULL,
        PRIMARY KEY (organization_id, block_id, level, position)
    ) WITHOUT ROWID;

    CREATE TABLE push_key (
        id              INTEGER PRIMARY KEY CHECK (id = 1),
        pkcs8           BLOB NOT NULL
    );

    CREATE TABLE push_endpoints (
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        address         TEXT NOT NULL,
        service         TEXT NOT NULL,
        p256dh          BLOB NOT NULL,
        auth            BLOB NOT NULL,
        failures        INTEGER NOT NULL DEFAULT 0,
        failing_since   INTEGER,
        PRIMARY KEY (account_id, organization_id, address)
    );

    CREATE INDEX push_endpoints_address ON push_endpoints (address);
    CREATE INDEX push_endpoints_organization ON push_endpoints (organization_id);

    CREATE TABLE mutes (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        mode            TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id, account_id)
    );

    CREATE TABLE told_keys (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        key             BLOB NOT NULL,
        PRIMARY KEY (organization_id, block_id, account_id, key)
    );
";
