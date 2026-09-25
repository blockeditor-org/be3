use crate::ui::{Line, LineStyle};

pub(super) fn lines() -> Vec<Line> {
    let mut lines = Vec::new();
    let status = crate::be::status();
    push(&mut lines, LineStyle::Heading, 0, "Block Stack");
    if let Some((account, workspace)) = crate::be::identity() {
        field(&mut lines, 1, "Account", account);
        field(&mut lines, 1, "Workspace", workspace);
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
        if status.connected {
            "Connected"
        } else {
            "Disconnected"
        },
    );
    field(&mut lines, 1, "Graph", graph());
    field(&mut lines, 1, "Live blocks", status.blocks);
    field(&mut lines, 1, "Worker wake-ups", status.wakes);
    field(
        &mut lines,
        1,
        "Changes",
        if status.unsealed == 0 {
            "Sealed".to_owned()
        } else {
            format!("{} block(s) unsealed", status.unsealed)
        },
    );
    field(
        &mut lines,
        1,
        "Error",
        status.error.as_deref().unwrap_or("None"),
    );
    lines
}

fn graph() -> String {
    if !crate::be::graph_loaded() {
        return "Loading".to_owned();
    }
    let roots = crate::be::query(crate::be::Query::Roots).len();
    let detached = crate::be::query(crate::be::Query::Detached).len();
    format!("{roots} root(s), {detached} detached")
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
