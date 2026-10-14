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
        password_hash   TEXT NOT NULL
    );

    CREATE TABLE sessions (
        token_hash      TEXT PRIMARY KEY,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
    );

    CREATE TABLE workspaces (
        id              TEXT PRIMARY KEY,
        name            TEXT NOT NULL,
        owner_id        TEXT NOT NULL REFERENCES accounts(id)
    );

    CREATE TABLE memberships (
        workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        role            TEXT NOT NULL,
        PRIMARY KEY (workspace_id, account_id)
    );

    CREATE TABLE invitations (
        id              TEXT PRIMARY KEY,
        workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        email           TEXT NOT NULL,
        role            TEXT NOT NULL,
        invited_by      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
    );

    CREATE TABLE blocks (
        workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        id              TEXT NOT NULL,
        content_type    TEXT NOT NULL,
        author          TEXT NOT NULL,
        parent_kind     INTEGER NOT NULL,
        parent_id       TEXT,
        head            TEXT,
        metadata        BLOB NOT NULL DEFAULT x'',
        version         INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (workspace_id, id)
    );

    CREATE TABLE block_edges (
        workspace_id    TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        reference_id    TEXT NOT NULL,
        PRIMARY KEY (workspace_id, block_id, reference_id)
    );

    CREATE TABLE block_access (
        workspace_id    TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        account_id      TEXT NOT NULL,
        access          TEXT NOT NULL,
        PRIMARY KEY (workspace_id, block_id, account_id)
    );

    CREATE TABLE block_commits (
        workspace_id    TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        commit_id       TEXT NOT NULL,
        parents         TEXT NOT NULL DEFAULT '',
        time            INTEGER NOT NULL,
        pinned          INTEGER NOT NULL CHECK (pinned IN (0, 1)),
        PRIMARY KEY (workspace_id, block_id, commit_id)
    );

    CREATE TABLE commit_objects (
        workspace_id    TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        commit_id       TEXT NOT NULL,
        position        INTEGER NOT NULL,
        hash            TEXT NOT NULL,
        PRIMARY KEY (workspace_id, block_id, commit_id, position)
    );

    CREATE TABLE block_objects (
        workspace_id    TEXT NOT NULL,
        block_id        TEXT NOT NULL,
        hash            TEXT NOT NULL,
        PRIMARY KEY (workspace_id, block_id, hash)
    );

    CREATE TABLE object_refs (
        hash            TEXT PRIMARY KEY,
        count           INTEGER NOT NULL CHECK (count >= 0)
    );

    CREATE TABLE recovery_keys (
        account_id      TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
        public          BLOB NOT NULL
    );

    CREATE TABLE workspace_keys (
        workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        sealed          BLOB NOT NULL,
        PRIMARY KEY (workspace_id, account_id)
    );

    CREATE INDEX block_edges_reference
        ON block_edges (workspace_id, reference_id);

    CREATE TABLE agents (
        account_id      TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
        workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        created_by      TEXT NOT NULL
    );

    CREATE INDEX commit_objects_hash
        ON commit_objects (workspace_id, hash);

    CREATE INDEX block_objects_hash
        ON block_objects (workspace_id, hash);
";
