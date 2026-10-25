use rusqlite::Connection;

use crate::ServerError;

pub fn initialize(connection: &Connection) -> Result<(), ServerError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    sqlite_schema::ensure(connection, SCHEMA)?;
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
        expires INTEGER NOT NULL
    );
    CREATE INDEX account_mail_email ON account_mail(email, purpose);

    CREATE TABLE organizations (
        id              TEXT PRIMARY KEY,
        name            TEXT NOT NULL,
        owner_id        TEXT REFERENCES accounts(id),
        kind            TEXT NOT NULL DEFAULT 'company'
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
        PRIMARY KEY (organization_id, id)
    );

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

    CREATE TABLE block_commits (
        organization_id TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        commit_id       TEXT NOT NULL,
        parents         TEXT NOT NULL DEFAULT '',
        time            INTEGER NOT NULL,
        pinned          INTEGER NOT NULL CHECK (pinned IN (0, 1)),
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
        PRIMARY KEY (organization_id, block_id, hash)
    );

    CREATE TABLE object_refs (
        hash            TEXT PRIMARY KEY,
        count           INTEGER NOT NULL CHECK (count >= 0)
    );

    CREATE TABLE recovery_keys (
        account_id      TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
        public          BLOB NOT NULL
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
        created_by      TEXT NOT NULL
    );

    CREATE INDEX commit_objects_hash
        ON commit_objects (organization_id, hash);

    CREATE INDEX block_objects_hash
        ON block_objects (organization_id, hash);

    CREATE TABLE server_account (
        id              INTEGER PRIMARY KEY CHECK (id = 1),
        account_id      TEXT NOT NULL REFERENCES accounts(id),
        secret          BLOB NOT NULL
    );
";
