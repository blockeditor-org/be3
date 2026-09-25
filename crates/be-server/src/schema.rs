use rusqlite::Connection;

use crate::ServerError;

pub fn initialize(connection: &Connection) -> Result<(), ServerError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,
            email           TEXT NOT NULL UNIQUE,
            display_name    TEXT NOT NULL,
            password_hash   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            token_hash      TEXT PRIMARY KEY,
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS workspaces (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            owner_id        TEXT NOT NULL REFERENCES accounts(id)
        );

        CREATE TABLE IF NOT EXISTS memberships (
            workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
            role            TEXT NOT NULL CHECK (role IN ('administrator', 'editor')),
            PRIMARY KEY (workspace_id, account_id)
        );

        CREATE TABLE IF NOT EXISTS invitations (
            id              TEXT PRIMARY KEY,
            workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            email           TEXT NOT NULL,
            role            TEXT NOT NULL CHECK (role IN ('administrator', 'editor')),
            invited_by      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS blocks (
            workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            id              TEXT NOT NULL,
            content_type    TEXT NOT NULL,
            author          TEXT NOT NULL,
            parent_kind     INTEGER NOT NULL CHECK (parent_kind IN (0, 1, 2)),
            parent_id       TEXT,
            head            TEXT,
            metadata        BLOB NOT NULL DEFAULT x'',
            version         INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (workspace_id, id)
        );

        CREATE TABLE IF NOT EXISTS block_edges (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            reference_id    TEXT NOT NULL,
            PRIMARY KEY (workspace_id, block_id, reference_id)
        );

        CREATE TABLE IF NOT EXISTS block_access (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            account_id      TEXT NOT NULL,
            access          TEXT NOT NULL CHECK (access IN ('know_exists', 'view', 'edit')),
            PRIMARY KEY (workspace_id, block_id, account_id)
        );

        CREATE TABLE IF NOT EXISTS block_commits (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            commit_id       TEXT NOT NULL,
            time            INTEGER NOT NULL,
            pinned          INTEGER NOT NULL CHECK (pinned IN (0, 1)),
            PRIMARY KEY (workspace_id, block_id, commit_id)
        );

        CREATE TABLE IF NOT EXISTS commit_objects (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            commit_id       TEXT NOT NULL,
            position        INTEGER NOT NULL,
            hash            TEXT NOT NULL,
            PRIMARY KEY (workspace_id, block_id, commit_id, position)
        );

        CREATE TABLE IF NOT EXISTS object_refs (
            hash            TEXT PRIMARY KEY,
            count           INTEGER NOT NULL CHECK (count >= 0)
        );

        CREATE INDEX IF NOT EXISTS block_edges_reference
            ON block_edges (workspace_id, reference_id);
        ",
    )?;
    Ok(())
}
