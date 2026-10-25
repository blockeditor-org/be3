use std::path::Path;

use wasi_threads::Mutex;

use be_store::StoreError;
use rusqlite::{Connection, params};

use super::{Row, Scope, Table};

const SCHEMA: &str = "
    CREATE TABLE refs (
        server       TEXT NOT NULL,
        account      BLOB NOT NULL,
        organization BLOB NOT NULL,
        kind         INTEGER NOT NULL,
        key          BLOB NOT NULL,
        value        BLOB NOT NULL,
        PRIMARY KEY (server, account, organization, kind, key)
    ) WITHOUT ROWID;
";

pub(crate) struct SqliteRefs {
    connection: Mutex<Connection>,
}

impl SqliteRefs {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::with_connection(Connection::open(path).map_err(failed)?)
    }

    pub(crate) fn in_memory() -> Result<Self, StoreError> {
        Self::with_connection(Connection::open_in_memory().map_err(failed)?)
    }

    fn with_connection(connection: Connection) -> Result<Self, StoreError> {
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(failed)?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(failed)?;
        sqlite_schema::ensure(&connection, SCHEMA).map_err(failed)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

impl SqliteRefs {
    pub(crate) fn load(&self, scope: &Scope) -> Result<Vec<Row>, StoreError> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection
            .prepare_cached(
                "SELECT kind, key, value FROM refs
                 WHERE server = ?1 AND account = ?2 AND organization = ?3",
            )
            .map_err(failed)?;
        let rows = statement
            .query_map(
                params![
                    scope.server,
                    scope.account.as_bytes().as_slice(),
                    scope.organization.as_bytes().as_slice()
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .map_err(failed)?;
        let mut loaded = Vec::new();
        for row in rows {
            let (kind, key, value) = row.map_err(failed)?;
            let Some(table) = u8::try_from(kind).ok().and_then(Table::from_code) else {
                continue;
            };
            loaded.push(Row {
                table,
                key,
                value: Some(value),
            });
        }
        Ok(loaded)
    }

    pub(crate) fn write(&self, scope: &Scope, rows: &[Row]) -> Result<(), StoreError> {
        let connection = self.connection.lock().unwrap();
        let transaction = connection.unchecked_transaction().map_err(failed)?;
        {
            let mut upsert = transaction
                .prepare_cached(
                    "INSERT OR REPLACE INTO refs (server, account, organization, kind, key, value)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(failed)?;
            let mut delete = transaction
                .prepare_cached(
                    "DELETE FROM refs
                     WHERE server = ?1 AND account = ?2 AND organization = ?3 AND kind = ?4
                       AND key = ?5",
                )
                .map_err(failed)?;
            for row in rows {
                let account = scope.account.as_bytes().as_slice();
                let organization = scope.organization.as_bytes().as_slice();
                let kind = i64::from(row.table.code());
                match &row.value {
                    Some(value) => upsert
                        .execute(params![
                            scope.server,
                            account,
                            organization,
                            kind,
                            row.key,
                            value
                        ])
                        .map_err(failed)?,
                    None => delete
                        .execute(params![scope.server, account, organization, kind, row.key])
                        .map_err(failed)?,
                };
            }
        }
        transaction.commit().map_err(failed)
    }
}

fn failed(error: impl std::fmt::Display) -> StoreError {
    StoreError::Io(std::io::Error::other(error.to_string()))
}
