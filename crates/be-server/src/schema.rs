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
        email           TEXT NOT NULL UNIQUE,
        display_name    TEXT NOT NULL,
        password_hash   TEXT NOT NULL,
        local_profile   INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE sessions (
        token_hash      TEXT PRIMARY KEY,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        kind            TEXT NOT NULL DEFAULT 'legacy',
        public_id       TEXT NOT NULL DEFAULT '',
        created         INTEGER NOT NULL DEFAULT 0,
        expires         INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE account_mail (
        token_hash TEXT PRIMARY KEY,
        purpose TEXT NOT NULL,
        account_id TEXT NOT NULL DEFAULT '',
        email TEXT NOT NULL,
        display_name TEXT NOT NULL DEFAULT '',
        password_hash TEXT NOT NULL DEFAULT '',
        device_code TEXT NOT NULL DEFAULT '',
        expires INTEGER NOT NULL,
        address TEXT NOT NULL DEFAULT '',
        invite_hash TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX account_mail_email ON account_mail(email, purpose);
    CREATE INDEX account_mail_address ON account_mail(address);

    CREATE TABLE organizations (
        id              TEXT PRIMARY KEY,
        name            TEXT NOT NULL,
        owner_id        TEXT REFERENCES accounts(id),
        kind            TEXT NOT NULL DEFAULT 'company',
        change_sequence INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE memberships (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        role            TEXT NOT NULL,
        PRIMARY KEY (organization_id, account_id)
    );

    CREATE TABLE invitations (
        id              TEXT PRIMARY KEY,
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        email           TEXT NOT NULL,
        role            TEXT NOT NULL,
        invited_by      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        sealed          BLOB NOT NULL DEFAULT x'',
        code            BLOB NOT NULL DEFAULT x'',
        escrow          BLOB NOT NULL DEFAULT x''
    );

    CREATE TABLE contact_links (
        id              TEXT PRIMARY KEY,
        owner_id        TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        card            BLOB NOT NULL,
        code            BLOB NOT NULL,
        grant_organization TEXT NOT NULL DEFAULT '',
        grant_block     TEXT NOT NULL DEFAULT '',
        grant_access    TEXT NOT NULL DEFAULT '',
        grant_key       TEXT NOT NULL DEFAULT '',
        grant_sealed    BLOB NOT NULL DEFAULT x'',
        accepter_id     TEXT NOT NULL DEFAULT '',
        accepter_card   BLOB NOT NULL DEFAULT x''
    );

    CREATE INDEX contact_links_owner
        ON contact_links (owner_id);

    CREATE TABLE contacts (
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        contact_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        sealed          BLOB NOT NULL DEFAULT x'',
        PRIMARY KEY (account_id, contact_id)
    );

    CREATE TABLE key_changes (
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        contact_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        position        INTEGER NOT NULL,
        sealed          BLOB NOT NULL,
        PRIMARY KEY (account_id, contact_id, position)
    );

    CREATE INDEX key_changes_contact
        ON key_changes (contact_id);

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

    CREATE INDEX blocks_changed
        ON blocks (organization_id, changed, id);

    CREATE TABLE block_departures (
        organization_id TEXT NOT NULL,
        root            TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        changed         INTEGER NOT NULL,
        time            INTEGER NOT NULL,
        PRIMARY KEY (organization_id, root, block_id)
    );

    CREATE INDEX block_departures_changed
        ON block_departures (organization_id, root, changed, block_id);

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
        signature       BLOB NOT NULL DEFAULT x'',
        PRIMARY KEY (organization_id, id)
    );

    CREATE TABLE pinned_escrows (
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        sealed          BLOB NOT NULL,
        PRIMARY KEY (account_id, organization_id)
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
    CREATE INDEX invitations_organization
        ON invitations (organization_id);
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
        generation      INTEGER NOT NULL,
        base_commit     TEXT,
        base_sequence   INTEGER NOT NULL,
        base_state      BLOB NOT NULL,
        lost            INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (organization_id, block_id)
    );

    CREATE TABLE journal_entries (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        sequence        INTEGER NOT NULL,
        generation      INTEGER NOT NULL,
        payload         BLOB NOT NULL,
        PRIMARY KEY (organization_id, block_id, sequence)
    );

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

    CREATE TABLE notify_modes (
        organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        mode            TEXT NOT NULL,
        PRIMARY KEY (organization_id, block_id, account_id)
    );
";
