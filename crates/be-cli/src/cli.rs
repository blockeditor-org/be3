use be_store::ObjectStore;
use serde_json::{Value, json};

use crate::tracker::{Body, DatabaseInfo, Filter, RowView, Tracker};

pub const USAGE: &str = "\
Usage:
  be databases                      List the databases this account can see
  be list DATABASE [FILTER]...      List a database's rows; FILTER is FIELD=VALUE,
                                    FIELD!=VALUE or FIELD~TEXT
  be show ID                        Show a row, its fields and its page
  be new DATABASE TITLE [FIELD=VALUE]... [--body TEXT]
                                    Add a row, and its page when --body is given
  be set ID [FIELD=VALUE]... [--body TEXT] [--append TEXT]
                                    Change a row's fields, replace its page or add to it
  be edit ID                        Edit a row's page in $EDITOR
  be whoami                         Say which account and workspace this is

  DATABASE is a database's name, its id prefix (BE) or its block id, and ID a
  row's id, such as BE-7K3Q2M. An empty VALUE clears the field. TEXT given as -
  is read from standard input. --json prints JSON instead of text.

Environment:
  BE3_RECOVERY_PHRASE   The recovery phrase the app showed when it added this agent
  BE3_TOKEN             The agent's token; without it the connection must be signed in
                        by a proxy that adds an Authorization: Bearer header
  BE3_WORKSPACE         The workspace, when the account is in more than one
  BE3_SERVER            The server (default: https://blocks.pfg.pw)";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    WhoAmI,
    Databases,
    List {
        database: String,
        filters: Vec<Filter>,
    },
    Show {
        id: String,
    },
    New {
        database: String,
        title: String,
        sets: Vec<(String, String)>,
        body: Body,
    },
    Set {
        id: String,
        sets: Vec<(String, String)>,
        body: Body,
    },
    Edit {
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub command: Command,
    pub json: bool,
}

pub fn parse(
    arguments: Vec<String>,
    mut stdin: impl FnMut() -> Result<String, String>,
) -> Result<Invocation, String> {
    let json = arguments.iter().any(|argument| argument == "--json");
    let mut rest = arguments
        .into_iter()
        .filter(|argument| argument != "--json")
        .collect::<Vec<_>>()
        .into_iter();
    let name = rest.next().unwrap_or_else(|| "help".to_owned());
    let mut positional = Vec::new();
    let mut body = Body::Keep;
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--body" | "--append" => {
                let text = rest
                    .next()
                    .ok_or_else(|| format!("{argument} needs the text to write"))?;
                let text = match text.as_str() {
                    "-" => stdin()?,
                    _ => text,
                };
                body = match argument.as_str() {
                    "--body" => Body::Replace(text),
                    _ => Body::Append(text),
                };
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positional.push(argument),
        }
    }
    let assignments = |items: &[String]| -> Result<Vec<(String, String)>, String> {
        items
            .iter()
            .map(|item| {
                item.split_once('=')
                    .map(|(field, value)| (field.trim().to_owned(), value.to_owned()))
                    .ok_or_else(|| format!("{item} is not FIELD=VALUE"))
            })
            .collect()
    };
    let needs = |count: usize, usage: &str| match positional.len() >= count {
        true => Ok(()),
        false => Err(format!("usage: be {usage}")),
    };
    let command = match name.as_str() {
        "help" | "--help" | "-h" => Command::Help,
        "whoami" => Command::WhoAmI,
        "databases" => Command::Databases,
        "list" => {
            needs(1, "list DATABASE [FILTER]...")?;
            Command::List {
                database: positional[0].clone(),
                filters: positional[1..]
                    .iter()
                    .map(|filter| Filter::parse(filter))
                    .collect::<Result<_, _>>()?,
            }
        }
        "show" => {
            needs(1, "show ID")?;
            Command::Show {
                id: positional[0].clone(),
            }
        }
        "new" => {
            needs(2, "new DATABASE TITLE [FIELD=VALUE]... [--body TEXT]")?;
            Command::New {
                database: positional[0].clone(),
                title: positional[1].clone(),
                sets: assignments(&positional[2..])?,
                body,
            }
        }
        "set" => {
            needs(1, "set ID [FIELD=VALUE]... [--body TEXT] [--append TEXT]")?;
            Command::Set {
                id: positional[0].clone(),
                sets: assignments(&positional[1..])?,
                body,
            }
        }
        "edit" => {
            needs(1, "edit ID")?;
            Command::Edit {
                id: positional[0].clone(),
            }
        }
        other => return Err(format!("unknown command {other}\n\n{USAGE}")),
    };
    Ok(Invocation { command, json })
}

pub async fn run<S: ObjectStore>(
    tracker: &Tracker<S>,
    invocation: &Invocation,
) -> Result<String, String> {
    let json = invocation.json;
    Ok(match &invocation.command {
        Command::Help => USAGE.to_owned(),
        Command::WhoAmI => {
            let peer = tracker.peer();
            let text = format!(
                "account {}\nworkspace {}",
                peer.account(),
                peer.workspace()
            );
            match json {
                true => json!({"account": peer.account(), "workspace": peer.workspace()})
                    .to_string(),
                false => text,
            }
        }
        Command::Databases => {
            let databases = tracker.databases().await?;
            match json {
                true => Value::Array(databases.iter().map(database_json).collect()).to_string(),
                false => databases
                    .iter()
                    .map(|database| {
                        format!(
                            "{}\t{}\t{} rows\t{}",
                            database.name,
                            match database.prefix.as_str() {
                                "" => "-",
                                prefix => prefix,
                            },
                            database.rows,
                            database.id
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            }
        }
        Command::List { database, filters } => {
            let database = tracker.find_database(database).await?;
            let rows = tracker.list(database, filters).await?;
            match json {
                true => Value::Array(rows.iter().map(row_json).collect()).to_string(),
                false => rows.iter().map(row_line).collect::<Vec<_>>().join("\n"),
            }
        }
        Command::Show { id } => shown(&tracker.show(id).await?, json),
        Command::New {
            database,
            title,
            sets,
            body,
        } => {
            let database = tracker.find_database(database).await?;
            shown(
                &tracker.create(database, title, sets, body.clone()).await?,
                json,
            )
        }
        Command::Set { id, sets, body } => {
            shown(&tracker.update(id, sets, body.clone()).await?, json)
        }
        Command::Edit { .. } => {
            return Err("be edit runs an editor, so only the be command does it".to_owned());
        }
    })
}

fn shown(row: &RowView, json: bool) -> String {
    match json {
        true => row_json(row).to_string(),
        false => row_text(row),
    }
}

pub fn row_line(row: &RowView) -> String {
    let fields = row
        .fields
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("; ");
    match fields.is_empty() {
        true => format!("{}\t{}", row.id, row.title),
        false => format!("{}\t{}\t[{}]", row.id, row.title, fields),
    }
}

pub fn row_text(row: &RowView) -> String {
    let mut text = format!("{}  {}\ndatabase: {}\n", row.id, row.title, row.database);
    for (name, value) in &row.fields {
        text.push_str(&format!("{name}: {value}\n"));
    }
    if let Some(body) = &row.body {
        text.push('\n');
        text.push_str(body);
    }
    text.trim_end().to_owned()
}

fn row_json(row: &RowView) -> Value {
    json!({
        "id": row.id,
        "database": row.database,
        "title": row.title,
        "fields": row
            .fields
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect::<serde_json::Map<_, _>>(),
        "page": row.page,
        "body": row.body,
    })
}

fn database_json(database: &DatabaseInfo) -> Value {
    json!({
        "id": database.id,
        "name": database.name,
        "prefix": database.prefix,
        "rows": database.rows,
    })
}

const EDIT_MARK: &str = "#!be ";

pub fn edit_document(row: &RowView) -> String {
    let mut text = format!(
        "{EDIT_MARK}{} {}\n{EDIT_MARK}Edit the page below. Lines that start with {EDIT_MARK}are left out.\n",
        row.id, row.title
    );
    text.push_str(row.body.as_deref().unwrap_or_default());
    text
}

pub fn edited_body(document: &str) -> String {
    let mut body = document
        .lines()
        .filter(|line| !line.starts_with(EDIT_MARK))
        .collect::<Vec<_>>()
        .join("\n");
    if document.ends_with('\n') && !body.is_empty() {
        body.push('\n');
    }
    body
}
