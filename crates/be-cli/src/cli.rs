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
  be import DATABASE FILE [--page COLUMN]
                                    Add a row for each line of a CSV file, such as
                                    Linear's export: Title is the title, COLUMN
                                    (Description) the page, and a column named like
                                    one of the database's sets it; an option an Enum
                                    column lacks is added
  be whoami                         Say which account and workspace this is
  be pull-request EVENT-FILE        Move the rows a GitHub pull request names, from the
                                    event GitHub Actions writes to $GITHUB_EVENT_PATH:
                                    opening it sets the status to $BE3_REVIEW_STATUS (In
                                    review) and links it on the row's page, merging it
                                    sets $BE3_DONE_STATUS (Done); $BE3_STATUS_FIELD names
                                    the column (Status)

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
    PullRequest {
        event: String,
    },
    Import {
        database: String,
        file: String,
        page: String,
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
    let mut page = "Description".to_owned();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--page" => {
                page = rest
                    .next()
                    .ok_or("--page needs the name of the column that holds the page")?;
            }
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
        "import" => {
            needs(2, "import DATABASE FILE [--page COLUMN]")?;
            Command::Import {
                database: positional[0].clone(),
                file: positional[1].clone(),
                page,
            }
        }
        "pull-request" => {
            needs(1, "pull-request EVENT-FILE")?;
            Command::PullRequest {
                event: positional[0].clone(),
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
            let text = format!("account {}\nworkspace {}", peer.account(), peer.workspace());
            match json {
                true => {
                    json!({"account": peer.account(), "workspace": peer.workspace()}).to_string()
                }
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
        Command::Import {
            database,
            file,
            page,
        } => {
            let text = std::fs::read_to_string(file)
                .map_err(|error| format!("could not read {file}: {error}"))?;
            let records = crate::csv::parse(&text)?;
            let database = tracker.find_database(database).await?;
            let imported = tracker.import(database, records, page).await?;
            let mut said = vec![format!(
                "added {} rows, {} with a page",
                imported.rows, imported.pages
            )];
            if !imported.options.is_empty() {
                said.push(format!("added options {}", imported.options.join(", ")));
            }
            if !imported.ignored.is_empty() {
                said.push(format!(
                    "left out the columns no field is named after: {}",
                    imported.ignored.join(", ")
                ));
            }
            said.extend(imported.problems);
            match json {
                true => json!({
                    "rows": imported.rows,
                    "pages": imported.pages,
                    "options": imported.options,
                    "ignored": imported.ignored,
                })
                .to_string(),
                false => said.join("\n"),
            }
        }
        Command::PullRequest { event } => {
            let text = std::fs::read_to_string(event)
                .map_err(|error| format!("could not read {event}: {error}"))?;
            let event: Value = serde_json::from_str(&text)
                .map_err(|error| format!("{event} is not a GitHub event: {error}"))?;
            let setting = |name: &str, default: &str| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| default.to_owned())
            };
            let statuses = Statuses {
                field: setting("BE3_STATUS_FIELD", "Status"),
                review: setting("BE3_REVIEW_STATUS", "In review"),
                done: setting("BE3_DONE_STATUS", "Done"),
            };
            pull_request(tracker, &event, &statuses).await?.join("\n")
        }
    })
}

pub struct Statuses {
    pub field: String,
    pub review: String,
    pub done: String,
}

pub async fn pull_request<S: ObjectStore>(
    tracker: &Tracker<S>,
    event: &Value,
    statuses: &Statuses,
) -> Result<Vec<String>, String> {
    let text = |pointer: &str| {
        event
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let action = text("/action");
    let url = text("/pull_request/html_url");
    let merged = event
        .pointer("/pull_request/merged")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let draft = event
        .pointer("/pull_request/draft")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mentioned = [
        text("/pull_request/head/ref"),
        text("/pull_request/title"),
        text("/pull_request/body"),
    ]
    .join("\n");
    let mut said = Vec::new();
    for id in row_ids(&mentioned) {
        let Ok(row) = tracker.show(&id).await else {
            continue;
        };
        let (status, link) = match action.as_str() {
            "closed" if merged => (Some(&statuses.done), false),
            "opened" | "reopened" | "ready_for_review" if !draft => (Some(&statuses.review), true),
            _ => (None, false),
        };
        let Some(status) = status else {
            continue;
        };
        let linked = row.body.as_deref().unwrap_or_default().contains(&url);
        let body = match link && !linked && !url.is_empty() {
            true => Body::Append(format!("Pull request: {url}\n")),
            false => Body::Keep,
        };
        match tracker
            .update(&row.id, &[(statuses.field.clone(), status.clone())], body)
            .await
        {
            Ok(_) => said.push(format!("{}: {} is now {status}", row.id, statuses.field)),
            Err(error) => said.push(format!("{}: {error}", row.id)),
        }
    }
    Ok(said)
}

pub fn row_ids(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for word in
        text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
    {
        let parts: Vec<&str> = word.split('-').collect();
        for pair in parts.windows(2) {
            let (prefix, code) = (pair[0], pair[1]);
            if prefix.is_empty()
                || !prefix
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
                || code.len() != 6
                || !code
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
            {
                continue;
            }
            let id = format!(
                "{}-{}",
                prefix.to_ascii_uppercase(),
                code.to_ascii_uppercase()
            );
            if !found.contains(&id) {
                found.push(id);
            }
        }
    }
    found
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
