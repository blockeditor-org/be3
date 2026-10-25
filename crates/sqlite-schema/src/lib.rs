use std::{collections::BTreeMap, error::Error, fmt};

use rusqlite::Connection;

#[derive(Debug)]
pub enum SchemaError {
    Database(rusqlite::Error),
    Mismatch(Vec<String>),
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "{error}"),
            Self::Mismatch(problems) => write!(
                formatter,
                "the database was made by an incompatible build: {}",
                problems.join("; ")
            ),
        }
    }
}

impl Error for SchemaError {}

impl From<rusqlite::Error> for SchemaError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

#[derive(PartialEq)]
struct Column {
    kind: String,
    not_null: bool,
    default: Option<String>,
    primary_key: i64,
}

struct Object {
    kind: String,
    table: String,
    sql: String,
}

pub fn open(path: impl AsRef<std::path::Path>) -> rusqlite::Result<Connection> {
    #[cfg(target_os = "wasi")]
    {
        // SQLite's WASI build locks a database with a lock directory beside it,
        // which a page that reloads mid-transaction leaves behind, and every
        // later open is then busy forever. One tab owns the files (the page's
        // `block-files` Web Lock) and each database has one connection, so
        // no locking is needed.
        Connection::open_with_flags_and_vfs(path, rusqlite::OpenFlags::default(), "unix-none")
    }
    #[cfg(not(target_os = "wasi"))]
    {
        Connection::open(path)
    }
}

pub fn ensure(connection: &Connection, declared: &str) -> Result<(), SchemaError> {
    let wanted = Connection::open_in_memory()?;
    wanted.execute_batch(declared)?;
    let transaction = connection.unchecked_transaction()?;
    let existing = objects(&transaction)?;
    let mut problems = Vec::new();
    for (name, object) in objects(&wanted)? {
        let Some(found) = existing.get(&name) else {
            transaction.execute_batch(&object.sql)?;
            continue;
        };
        if found.kind != object.kind || found.table != object.table {
            problems.push(format!("{name} is a {} on {}", found.kind, found.table));
        } else if object.kind == "index" && found.sql != object.sql {
            transaction.execute_batch(&format!("DROP INDEX \"{name}\"; {}", object.sql))?;
        } else if object.kind == "table" {
            add_columns(&transaction, &wanted, &name, &mut problems)?;
        }
    }
    if !problems.is_empty() {
        return Err(SchemaError::Mismatch(problems));
    }
    transaction.commit()?;
    Ok(())
}

fn objects(connection: &Connection) -> Result<BTreeMap<String, Object>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql FROM sqlite_master
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'",
    )?;
    statement
        .query_map([], |row| {
            Ok((
                row.get(1)?,
                Object {
                    kind: row.get(0)?,
                    table: row.get(2)?,
                    sql: row.get(3)?,
                },
            ))
        })?
        .collect()
}

fn columns(
    connection: &Connection,
    table: &str,
) -> Result<BTreeMap<String, Column>, rusqlite::Error> {
    let mut statement = connection
        .prepare("SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?1)")?;
    statement
        .query_map([table], |row| {
            Ok((
                row.get(0)?,
                Column {
                    kind: row.get(1)?,
                    not_null: row.get(2)?,
                    default: row.get(3)?,
                    primary_key: row.get(4)?,
                },
            ))
        })?
        .collect()
}

fn add_columns(
    connection: &Connection,
    wanted: &Connection,
    table: &str,
    problems: &mut Vec<String>,
) -> Result<(), rusqlite::Error> {
    let found = columns(connection, table)?;
    let declared = columns(wanted, table)?;
    for name in found.keys().filter(|name| !declared.contains_key(*name)) {
        problems.push(format!("{table}.{name} is not a column this build knows"));
    }
    for (name, column) in &declared {
        match found.get(name) {
            Some(existing) if existing == column => {}
            Some(_) => problems.push(format!("{table}.{name} has a different type")),
            None if column.primary_key != 0 || (column.not_null && column.default.is_none()) => {
                problems.push(format!(
                    "{table}.{name} is missing and cannot be added to an existing table"
                ));
            }
            None => {
                let mut definition = format!("\"{name}\" {}", column.kind);
                if column.not_null {
                    definition.push_str(" NOT NULL");
                }
                if let Some(default) = &column.default {
                    definition.push_str(" DEFAULT ");
                    definition.push_str(default);
                }
                connection
                    .execute_batch(&format!("ALTER TABLE \"{table}\" ADD COLUMN {definition}"))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
