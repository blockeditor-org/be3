use be_block::BlockContent;
use be_graph::BlockParent;
use be_store::ObjectStore;
use database_types::database::DatabaseContent;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::blocks::Entry;
use crate::tracker::{Assignment, Body, Filter, NewColumn, RowView, Tracker};
use text_block_types::TextContent;

pub const USAGE: &str = "\
Usage: be TARGET [VERB] [ARGUMENTS]   or   be COMMAND

TARGET is a block, by the end of its id (`be ls` prints it), its full id or
its name. A row is its id, `be BE-7K3Q2M`, found through the prefixes of the
workspaces this account can see, or named within its database:
`be Issues row BE-7K3Q2M`.

Commands:
  be ls [TARGET]                      List the workspaces and what is shared with this
                                      account, or a block's children
  be workspaces                       List the workspaces, and which this account may write in
  be whoami                           Say which account and organization this is
  be help [TARGET]                    This, or the verbs a target takes
  be new-database NAME [--prefix BE] [--in TARGET]
                                      Make a database (and a list view of it) in TARGET,
                                      or in the one workspace this account may write in
  be upload FILE [--in TARGET]        Copy the blocks a download wrote into TARGET (or,
                                      as new-database, a workspace) with new ids
  be sync DIRECTORY [--watch [SECONDS]]
                                      Sync a directory with the folder it was synced with before

Any block:
  ls | info                           Its children, or where it is and who may change it
  delete                              Move it, and everything inside it, to Recently Deleted
  download [FILE]                     Write it, and everything inside it, to one file
                                      (NAME.block unless FILE is given) for `be upload`

A folder or a workspace:
  sync DIRECTORY [--watch [SECONDS]]  Keep DIRECTORY and the folder in step both ways:
                                      text, image, PDF, audio and other files as files,
                                      folders as directories; --watch keeps going, syncing
                                      on every change here and every SECONDS (10) for
                                      changes in block-app

A database (or a view of one):
  rows [FILTER]...                    Its rows; FILTER is FIELD=VALUE, FIELD!=VALUE or FIELD~TEXT
  new TITLE [FIELD=VALUE]... [--body TEXT]
                                      Add a row, with a page when --body is given
  schema                              Its columns, and each choice's colour, icon, whether
                                      it closes a row and which is the default
  option FIELD OPTION [SETTING=VALUE]...
                                      Change a choice: color=blue, icon=done, closed=yes,
                                      default=yes, name=NEW, after=OPTION or first=yes
  add-option FIELD OPTION [SETTING=VALUE]...
                                      Add a choice, with the same settings
  add-column NAME TYPE [--options A,B] [--links DATABASE] [--multiple] [--inverse NAME]
                                      TYPE is text, number, enum, boolean, date, color,
                                      block or relation
  import FILE [--page COLUMN]         Add a row for each line of a CSV file, such as
                                      Linear's export (Title, Description, ...)
  pull-request EVENT-FILE             Move the rows of this database a GitHub pull
                                      request names

A row (be ROW-ID [VERB], or be DATABASE row ROW-ID [VERB]):
  show                                Its fields and its page (what a bare row id does)
  set [FIELD=VALUE]... [--body TEXT] [--append TEXT]
                                      FIELD+=VALUE adds a link to a relation of several
                                      rows and FIELD-=VALUE removes one; an empty VALUE
                                      clears the field
  page [cat | write TEXT | append TEXT | edit]
                                      Its page, a text block made the first time it is written
  delete                              Delete the row and its page

A text block:
  cat | write TEXT | append TEXT | edit

A terminal block:
  terminal-host [-- PROGRAM ARGUMENTS...]
                                      Run its shell (or PROGRAM) on this machine, as this
                                      machine's user, for everyone the block is shared with:
                                      it stays connected and keeps the shell running until
                                      stopped. Share the block with this agent first.

TEXT given as - is read from standard input. --json prints JSON.

Environment:
  BE3_AGENT_CREDENTIALS JSON credentials copied from the app (replaces the four fields below)
  BE3_RECOVERY_PHRASE   The recovery phrase the app showed when it added this agent
  BE3_TOKEN             The agent's token
  BE3_ORGANIZATION      The organization, when the account is in more than one
  BE3_SERVER            The server (default: https://api.be3.pfg.pw)
  BE3_AUTHORIZATION     A Be3-Gateway-Authorization header a gateway in front of the server wants
  BE3_CACHE             Where it keeps the objects it reads (default: ~/.cache/be3/be)";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub body: Option<Body>,
    pub page: Option<String>,
    pub prefix: Option<String>,
    pub within: Option<String>,
    pub column: NewColumn,
    pub command: Vec<String>,
    pub watch: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub target: Option<String>,
    pub verb: String,
    pub arguments: Vec<String>,
    pub options: Options,
    pub json: bool,
}

pub enum Outcome {
    Print(String),
    Edit {
        block: Uuid,
        label: String,
        base: String,
    },
    Watch {
        root: Option<Uuid>,
        directory: std::path::PathBuf,
        every: std::time::Duration,
    },
}

const COMMANDS: [&str; 7] = [
    "ls",
    "workspaces",
    "whoami",
    "help",
    "new-database",
    "upload",
    "sync",
];

const WATCH_SECONDS: u64 = 10;

pub fn parse(
    arguments: Vec<String>,
    mut stdin: impl FnMut() -> Result<String, String>,
) -> Result<Invocation, String> {
    let mut options = Options::default();
    let mut json = false;
    let mut positional = Vec::new();
    let mut rest = arguments.into_iter();
    let mut text = |value: String| match value.as_str() {
        "-" => stdin(),
        _ => Ok(value),
    };
    while let Some(argument) = rest.next() {
        let mut value = || {
            rest.next()
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument.as_str() {
            "--" => {
                options.command = rest.by_ref().collect();
                break;
            }
            "--json" => json = true,
            "--watch" => {
                let seconds = match rest.as_slice().first().map(|next| next.parse::<u64>()) {
                    Some(Ok(seconds)) => {
                        rest.next();
                        seconds.max(1)
                    }
                    _ => WATCH_SECONDS,
                };
                options.watch = Some(seconds);
            }
            "--multiple" => options.column.multiple = true,
            "--body" => options.body = Some(Body::Replace(text(value()?)?)),
            "--append" => options.body = Some(Body::Append(text(value()?)?)),
            "--page" => options.page = Some(value()?),
            "--prefix" => options.prefix = Some(value()?),
            "--in" => options.within = Some(value()?),
            "--links" => options.column.links = Some(value()?),
            "--inverse" => options.column.inverse = value()?,
            "--options" => {
                options.column.options = value()?
                    .split(',')
                    .map(str::trim)
                    .filter(|option| !option.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positional.push(argument),
        }
    }
    let mut positional = positional.into_iter();
    let first = positional.next().unwrap_or_else(|| "help".to_owned());
    let (target, verb) = match COMMANDS.contains(&first.as_str()) || first == "--help" {
        true => (None, first.trim_start_matches("--").to_owned()),
        false => (Some(first), positional.next().unwrap_or_default()),
    };
    let mut arguments: Vec<String> = positional.collect();
    let action = match arguments.len() {
        0 | 1 => Some(&verb),
        length => arguments.get(length - 2),
    };
    if action.is_some_and(|action| matches!(action.as_str(), "write" | "append"))
        && let Some(last) = arguments.last_mut()
        && last == "-"
    {
        *last = text(std::mem::take(last))?;
    }
    Ok(Invocation {
        target,
        verb,
        arguments,
        options,
        json,
    })
}

pub async fn run<S: ObjectStore>(
    tracker: &Tracker<S>,
    invocation: &Invocation,
) -> Result<Outcome, String> {
    let Some(target) = &invocation.target else {
        if invocation.verb == "sync" {
            return sync(tracker, None, invocation).await;
        }
        return command(tracker, invocation).await.map(Outcome::Print);
    };
    if crate::tracker::looks_like_row(target) {
        let database = tracker.database_of_row(target).await?;
        let mut arguments = vec![target.clone()];
        arguments.extend((!invocation.verb.is_empty()).then(|| invocation.verb.clone()));
        arguments.extend(invocation.arguments.iter().cloned());
        let row = Invocation {
            verb: "row".to_owned(),
            arguments,
            ..invocation.clone()
        };
        return row_verb(tracker, database, &row).await;
    }
    let entry = tracker.resolve(target).await?;
    if invocation.verb == "row" {
        let database = tracker.database_of(&entry).await?;
        return row_verb(tracker, database, invocation).await;
    }
    block_verb(tracker, &entry, invocation).await
}

async fn command<S: ObjectStore>(
    tracker: &Tracker<S>,
    invocation: &Invocation,
) -> Result<String, String> {
    let json = invocation.json;
    let arguments = &invocation.arguments;
    match invocation.verb.as_str() {
        "help" => Ok(USAGE.to_owned()),
        "whoami" => {
            let peer = tracker.peer();
            Ok(match json {
                true => json!({"account": peer.account(), "organization": peer.organization()})
                    .to_string(),
                false => format!(
                    "account {}\norganization {}",
                    peer.account(),
                    peer.organization()
                ),
            })
        }
        "ls" => listing(tracker, None, json).await,
        "workspaces" => {
            let top = tracker.top().await?;
            let workspaces = crate::blocks::workspaces(&top);
            if json {
                return Ok(Value::Array(
                    workspaces
                        .iter()
                        .map(|entry| {
                            let mut described = entry_json(entry);
                            described["writable"] = Value::Bool(entry.access.can_edit());
                            described
                        })
                        .collect(),
                )
                .to_string());
            }
            if workspaces.is_empty() {
                return Ok("no workspaces; `be ls` lists what is shared with this account".into());
            }
            let width = workspaces
                .iter()
                .map(|entry| entry.short.len())
                .max()
                .unwrap_or(4);
            Ok(workspaces
                .iter()
                .map(|entry| {
                    let access = match entry.access.can_edit() {
                        true => "may write",
                        false => "may read",
                    };
                    format!("{:width$}  {:9}  {}", entry.short, access, entry.label())
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "new-database" => {
            let name = arguments
                .first()
                .ok_or("usage: be new-database NAME [--prefix BE] [--in TARGET]")?;
            let parent = within(tracker, invocation).await?;
            let prefix = invocation.options.prefix.clone().unwrap_or_default();
            let (view, database) = tracker.create_database(name, &prefix, parent).await?;
            Ok(format!(
                "made {name}: database {}, list view {}",
                crate::blocks::short_of(database),
                crate::blocks::short_of(view)
            ))
        }
        "upload" => {
            let file = arguments
                .first()
                .ok_or("usage: be upload FILE [--in TARGET]")?;
            let bytes =
                std::fs::read(file).map_err(|error| format!("could not read {file}: {error}"))?;
            let blocks = be_client::transfer::decode(&bytes)?;
            let parent = within(tracker, invocation).await?;
            let uploaded = tracker.upload(blocks, parent).await?;
            let top = tracker.entry(uploaded.top).await.map_or_else(
                |_| uploaded.top.to_string(),
                |entry| format!("{} ({})", entry.label(), entry.short),
            );
            let mut said = format!("uploaded {top}");
            if uploaded.blocks > 1 {
                said.push_str(&format!(" and {} blocks inside it", uploaded.blocks - 1));
            }
            Ok(said)
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}

async fn sync<S: ObjectStore>(
    tracker: &Tracker<S>,
    root: Option<Uuid>,
    invocation: &Invocation,
) -> Result<Outcome, String> {
    let directory = std::path::PathBuf::from(invocation.arguments.first().ok_or(
        "usage: be FOLDER sync DIRECTORY [--watch [SECONDS]], or be sync DIRECTORY once it has been synced",
    )?);
    if let Some(seconds) = invocation.options.watch {
        return Ok(Outcome::Watch {
            root,
            directory,
            every: std::time::Duration::from_secs(seconds),
        });
    }
    let report = be_sync::sync(
        tracker.peer(),
        &mut *tracker.types().await,
        &directory,
        &be_sync::Target { root },
    )
    .await?;
    tracker.invalidate().await;
    Ok(Outcome::Print(report.to_string()))
}

async fn within<S: ObjectStore>(
    tracker: &Tracker<S>,
    invocation: &Invocation,
) -> Result<BlockParent, String> {
    match &invocation.options.within {
        Some(within) => Ok(BlockParent::Block(tracker.resolve(within).await?.id)),
        None => Ok(BlockParent::Block(
            crate::blocks::default_parent(&tracker.top().await?)?.id,
        )),
    }
}

async fn listing<S: ObjectStore>(
    tracker: &Tracker<S>,
    parent: Option<Uuid>,
    json: bool,
) -> Result<String, String> {
    let mut children = match parent {
        Some(parent) => tracker.children(parent).await?,
        None => tracker.top().await?,
    };
    let pages = match parent {
        Some(parent)
            if tracker.entry(parent).await?.block_type == DatabaseContent::CONTENT_TYPE =>
        {
            tracker.row_pages(parent).await?
        }
        _ => Vec::new(),
    };
    children.retain(|entry| {
        !pages.contains(&entry.id) && !be_block::EDIT_KINDS.contains(&entry.block_type)
    });
    if json {
        return Ok(Value::Array(children.iter().map(entry_json).collect()).to_string());
    }
    if children.is_empty() && pages.is_empty() {
        return Ok("nothing here".to_owned());
    }
    let paged = match pages.len() {
        0 => String::new(),
        count => format!("\n…and {count} row pages, which `be ROW page` reads"),
    };
    let width = children
        .iter()
        .map(|entry| entry.short.len())
        .max()
        .unwrap_or(4);
    let inside =
        futures_util::future::try_join_all(children.iter().map(|entry| tracker.children(entry.id)))
            .await?;
    Ok(children
        .iter()
        .zip(inside)
        .map(|(entry, inside)| {
            let extra = match (
                &entry.prefix,
                entry.block_type == DatabaseContent::CONTENT_TYPE,
            ) {
                (Some(prefix), true) if !prefix.is_empty() => format!("  rows {prefix}-…"),
                _ => String::new(),
            };
            let inside = match inside.len() {
                0 => String::new(),
                count => format!("  ({count} inside)"),
            };
            format!(
                "{:width$}  {:13}  {}{extra}{inside}",
                entry.short,
                entry.kind(),
                entry.label()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + &paged)
}

fn entry_json(entry: &Entry) -> Value {
    json!({
        "id": entry.id,
        "short": entry.short,
        "kind": entry.kind(),
        "name": entry.label(),
        "prefix": entry.prefix,
    })
}

async fn block_verb<S: ObjectStore>(
    tracker: &Tracker<S>,
    entry: &Entry,
    invocation: &Invocation,
) -> Result<Outcome, String> {
    let json = invocation.json;
    let arguments = &invocation.arguments;
    let is_text = entry.block_type == TextContent::CONTENT_TYPE;
    let database = tracker.database_of(entry).await.ok();
    let verb = match (invocation.verb.as_str(), is_text, database) {
        ("", true, _) => "cat",
        ("", false, Some(_)) => "rows",
        ("", false, None) => "ls",
        (verb, _, _) => verb,
    };
    let print = |text: String| Ok(Outcome::Print(text));
    match (verb, is_text, database) {
        ("ls", _, _) => print(listing(tracker, Some(entry.id), json).await?),
        ("info", _, _) => {
            let parent = match entry.parent {
                BlockParent::Block(parent) if tracker.is_hidden(parent).await => {
                    "a block this account cannot see (it was shared from there)".to_owned()
                }
                BlockParent::Block(parent) => tracker.entry(parent).await.map_or_else(
                    |_| parent.to_string(),
                    |parent| format!("{} ({})", parent.label(), parent.short),
                ),
                BlockParent::Root => "the root".to_owned(),
                BlockParent::Detached => "nothing (deleted)".to_owned(),
            };
            print(format!(
                "{} ({})\nid {}\nkind {}\nin {parent}\naccess {:?}",
                entry.label(),
                entry.short,
                entry.id,
                entry.kind(),
                entry.access
            ))
        }
        ("help", _, _) => print(USAGE.to_owned()),
        ("terminal-host", _, _) => Err(
            "terminal-host needs a live connection; run it as `be TARGET terminal-host`".to_owned(),
        ),
        ("sync", _, _) => sync(tracker, Some(entry.id), invocation).await,
        ("delete", _, _) => print(tracker.delete_block(entry).await?),
        ("download", _, _) => {
            let (top, blocks) = tracker.download(entry).await?;
            let file = arguments
                .first()
                .cloned()
                .unwrap_or_else(|| be_client::transfer::file_name(&top.label()));
            let bytes = be_client::transfer::encode(&blocks);
            write_private(&file, &bytes)
                .map_err(|error| format!("could not write {file}: {error}"))?;
            let inside = match blocks.len() {
                1 => String::new(),
                count => format!(" and {} blocks inside it", count - 1),
            };
            print(format!(
                "wrote {} ({}){inside} to {file}, {} bytes; `be upload {file}` copies it into an organization",
                top.label(),
                top.short,
                bytes.len()
            ))
        }
        ("cat", true, _) => print(tracker.text(entry.id).await?),
        ("write" | "append", true, _) => {
            let text = arguments
                .first()
                .cloned()
                .ok_or_else(|| format!("usage: be {} {verb} TEXT", entry.short))?;
            let body = match verb {
                "write" => Body::Replace(text),
                _ => Body::Append(text),
            };
            tracker.write_text(entry.id, &body).await?;
            print(format!("wrote {}", entry.label()))
        }
        ("edit", true, _) => Ok(Outcome::Edit {
            block: entry.id,
            label: entry.label(),
            base: tracker.text(entry.id).await?,
        }),
        ("rows", _, Some(database)) => {
            let filters = arguments
                .iter()
                .map(|filter| Filter::parse(filter))
                .collect::<Result<Vec<_>, _>>()?;
            let rows = tracker.list(database, &filters).await?;
            print(match json {
                true => Value::Array(rows.iter().map(row_json).collect()).to_string(),
                false => rows.iter().map(row_line).collect::<Vec<_>>().join("\n"),
            })
        }
        ("new", _, Some(database)) => {
            let title = arguments
                .first()
                .ok_or_else(|| format!("usage: be {} new TITLE [FIELD=VALUE]...", entry.short))?;
            let assignments = assignments(&arguments[1..])?;
            let body = invocation.options.body.clone().unwrap_or(Body::Keep);
            let row = tracker.create(database, title, &assignments, body).await?;
            print(shown(&row, json))
        }
        ("schema", _, Some(database)) => print(tracker.schema(database).await?.join("\n")),
        (verb @ ("option" | "add-option"), _, Some(database)) => {
            let [field, option, settings @ ..] = arguments.as_slice() else {
                return Err(format!(
                    "usage: be {} {verb} FIELD OPTION [SETTING=VALUE]...",
                    entry.short
                ));
            };
            tracker
                .set_option(
                    database,
                    field,
                    option,
                    &assignments(settings)?,
                    verb == "add-option",
                )
                .await?;
            print(tracker.schema(database).await?.join("\n"))
        }
        ("add-column", _, Some(database)) => {
            let [name, kind] = arguments.as_slice() else {
                return Err(format!("usage: be {} add-column NAME TYPE", entry.short));
            };
            tracker
                .add_column(database, name, kind, &invocation.options.column)
                .await?;
            print(tracker.schema(database).await?.join("\n"))
        }
        ("import", _, Some(database)) => {
            let file = arguments
                .first()
                .ok_or_else(|| format!("usage: be {} import FILE", entry.short))?;
            let text = std::fs::read_to_string(file)
                .map_err(|error| format!("could not read {file}: {error}"))?;
            let records = crate::csv::parse(&text)?;
            let page = invocation
                .options
                .page
                .clone()
                .unwrap_or_else(|| "Description".to_owned());
            let imported = tracker.import(database, records, &page).await?;
            let mut said = vec![format!(
                "added {} rows, {} with a page",
                imported.rows, imported.pages
            )];
            if !imported.options.is_empty() {
                said.push(format!("added options {}", imported.options.join(", ")));
            }
            if !imported.linked_rows.is_empty() {
                said.push(format!(
                    "added rows to link to {}",
                    imported.linked_rows.join(", ")
                ));
            }
            if !imported.ignored.is_empty() {
                said.push(format!(
                    "left out the columns no field is named after: {}",
                    imported.ignored.join(", ")
                ));
            }
            said.extend(imported.problems);
            print(said.join("\n"))
        }
        ("pull-request", _, Some(database)) => {
            let event = arguments
                .first()
                .ok_or_else(|| format!("usage: be {} pull-request EVENT-FILE", entry.short))?;
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
                done: std::env::var("BE3_DONE_STATUS")
                    .ok()
                    .filter(|value| !value.trim().is_empty()),
            };
            print(
                pull_request(tracker, database, &event, &statuses)
                    .await?
                    .join("\n"),
            )
        }
        (verb, _, _) => Err(format!(
            "{} ({}) is a {}; it takes {}",
            entry.label(),
            entry.short,
            entry.kind(),
            match (is_text, database) {
                (true, _) => "cat, write, append, edit, ls, info, download and delete",
                (false, Some(_)) =>
                    "rows, row, new, schema, option, add-option, add-column, import, pull-request, ls, info, download and delete",
                (false, None) => "ls, info, download and delete",
            }
        ) + &format!(", not {verb}")),
    }
}

async fn row_verb<S: ObjectStore>(
    tracker: &Tracker<S>,
    database: Uuid,
    invocation: &Invocation,
) -> Result<Outcome, String> {
    let json = invocation.json;
    let (code, verb, arguments) = match invocation.arguments.as_slice() {
        [] => return Err("usage: be DATABASE row ROW-ID [VERB]".to_owned()),
        [code] => (code.as_str(), "", &[][..]),
        [code, verb, arguments @ ..] => (code.as_str(), verb.as_str(), arguments),
    };
    let print = |text: String| Ok(Outcome::Print(text));
    match verb {
        "" | "show" => print(shown(&tracker.show(database, code).await?, json)),
        "set" => {
            let body = invocation.options.body.clone().unwrap_or(Body::Keep);
            let row = tracker
                .update(database, code, &assignments(arguments)?, body)
                .await?;
            print(shown(&row, json))
        }
        "delete" => print(tracker.delete(database, code).await?),
        "page" => {
            let action = arguments.first().map_or("cat", String::as_str);
            let writing = matches!(action, "write" | "append" | "edit");
            let page = tracker.page_of(database, code, writing).await?;
            match (action, page) {
                ("cat", None) => print(String::new()),
                ("cat", Some(page)) => print(tracker.text(page).await?),
                ("write" | "append", Some(page)) => {
                    let text = arguments.get(1).cloned().ok_or_else(|| {
                        format!("usage: be DATABASE row {code} page {action} TEXT")
                    })?;
                    let body = match action {
                        "write" => Body::Replace(text),
                        _ => Body::Append(text),
                    };
                    tracker.write_text(page, &body).await?;
                    print(format!("wrote the page of {code}"))
                }
                ("edit", Some(page)) => Ok(Outcome::Edit {
                    block: page,
                    label: code.to_owned(),
                    base: tracker.text(page).await?,
                }),
                (other, _) => Err(format!(
                    "a page takes cat, write, append or edit, not {other}"
                )),
            }
        }
        other => Err(format!(
            "a row takes show, set, page and delete, not {other}"
        )),
    }
}

fn write_private(file: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(file)?.write_all(bytes)
}

fn assignments(arguments: &[String]) -> Result<Vec<Assignment>, String> {
    arguments
        .iter()
        .map(|text| Assignment::parse(text))
        .collect()
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
        .map(|field| match field.links.len() {
            0..=2 => format!("{}: {}", field.name, field.text),
            count => format!(
                "{}: {} and {} more",
                field.name,
                field.links[0].0,
                count - 1
            ),
        })
        .collect::<Vec<_>>()
        .join("; ");
    match fields.is_empty() {
        true => format!("{}\t{}", row.id, row.title),
        false => format!("{}\t{}\t[{}]", row.id, row.title, fields),
    }
}

pub fn row_text(row: &RowView) -> String {
    let mut text = format!("{}  {}\ndatabase: {}\n", row.id, row.title, row.database);
    for field in &row.fields {
        text.push_str(&format!("{}: {}\n", field.name, field.text));
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
            .map(|field| {
                let value = match field.links.is_empty() {
                    true => Value::String(field.text.clone()),
                    false => Value::Array(
                        field
                            .links
                            .iter()
                            .map(|(id, title)| json!({"id": id, "title": title}))
                            .collect(),
                    ),
                };
                (field.name.clone(), value)
            })
            .collect::<serde_json::Map<_, _>>(),
        "page": row.page,
        "body": row.body,
    })
}

const EDIT_MARK: &str = "#!be ";

pub fn edit_document(label: &str, body: &str) -> String {
    format!(
        "{EDIT_MARK}{label}\n{EDIT_MARK}Edit the text below. Lines that start with {EDIT_MARK}are left out.\n{body}"
    )
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

pub struct Statuses {
    pub field: String,
    pub review: String,
    pub done: Option<String>,
}

pub async fn pull_request<S: ObjectStore>(
    tracker: &Tracker<S>,
    database: Uuid,
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
    let prefix = tracker
        .read::<DatabaseContent>(database)
        .await?
        .map(|content| content.root().prefix)
        .unwrap_or_default();
    let mut said = Vec::new();
    for id in row_ids(&mentioned) {
        if !id
            .rsplit_once('-')
            .is_some_and(|(held, _)| held.eq_ignore_ascii_case(prefix.trim()))
        {
            continue;
        }
        let Ok(row) = tracker.show(database, &id).await else {
            continue;
        };
        let (status, link) = match action.as_str() {
            "closed" if merged => (
                Some(match &statuses.done {
                    Some(done) => done.clone(),
                    None => tracker
                        .closed_option(database, &statuses.field)
                        .await?
                        .unwrap_or_else(|| "Done".to_owned()),
                }),
                false,
            ),
            "opened" | "reopened" | "ready_for_review" if !draft => {
                (Some(statuses.review.clone()), true)
            }
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
            .update(
                database,
                &row.id,
                &[Assignment::set(&statuses.field, &status)],
                body,
            )
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
