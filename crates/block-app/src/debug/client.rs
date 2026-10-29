use crate::ui::{Line, LineStyle};

pub(super) fn lines() -> Vec<Line> {
    let mut lines = Vec::new();
    let status = block_app_stack::status();
    push(&mut lines, LineStyle::Heading, 0, "Block Stack");
    if let Some((account, organization)) = block_app_stack::identity() {
        field(&mut lines, 1, "Account", account);
        field(&mut lines, 1, "Organization", organization);
    }
    field(
        &mut lines,
        1,
        "Peer",
        if status.running { "Running" } else { "Off" },
    );
    field(
        &mut lines,
        1,
        "Connection",
        match (status.connected, status.syncing) {
            (true, true) => "Connected, syncing",
            (true, false) => "Connected",
            (false, _) => "Disconnected",
        },
    );
    field(&mut lines, 1, "Graph", graph());
    field(&mut lines, 1, "Live blocks", status.blocks);
    field(&mut lines, 1, "Worker wake-ups", status.wakes);
    field(
        &mut lines,
        1,
        "Changes",
        if status.unsaved == 0 {
            "Saved on this device".to_owned()
        } else {
            format!("{} not yet saved on this device", status.unsaved)
        },
    );
    field(&mut lines, 1, "Waiting to sync", status.waiting);
    field(&mut lines, 1, "Set aside", status.unsent);
    field(&mut lines, 1, "Merge conflicts", status.conflicts);
    for block in &status.conflicted {
        field(&mut lines, 2, "Conflicted", block);
    }
    field(
        &mut lines,
        1,
        "Error",
        status.error.as_deref().unwrap_or("None"),
    );
    lines
}

fn graph() -> String {
    if !block_app_stack::ready(block_app_stack::Query::Roots) {
        return "Loading".to_owned();
    }
    let roots = block_app_stack::query(block_app_stack::Query::Roots).len();
    format!("{roots} root(s)")
}

fn push(lines: &mut Vec<Line>, style: LineStyle, indent: u8, text: impl Into<String>) {
    lines.push(Line {
        text: text.into(),
        style,
        indent,
    });
}

fn field(lines: &mut Vec<Line>, indent: u8, label: &str, value: impl ToString) {
    push(
        lines,
        LineStyle::Code,
        indent,
        format!("{label}: {}", value.to_string()),
    );
}
