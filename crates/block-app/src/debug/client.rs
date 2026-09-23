use block_client::{BlockClient, ClientDebugEntry, ClientDebugSnapshot, properties};

use crate::ui::{Line, LineStyle};

pub(super) fn lines(client: &BlockClient) -> Vec<Line> {
    let mut lines = Vec::new();
    snapshot(&mut lines, &client.client_debug_snapshot());
    be_stack(&mut lines, &crate::be::status());
    lines
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

fn empty(lines: &mut Vec<Line>) {
    push(lines, LineStyle::Muted, 1, "None");
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

fn snapshot(lines: &mut Vec<Line>, snapshot: &ClientDebugSnapshot) {
    field(lines, 0, "Client", snapshot.client_id);
    field(lines, 0, "Account", snapshot.account_id);
    field(lines, 0, "Workspace", snapshot.workspace_id);
    field(
        lines,
        0,
        "Connection",
        if snapshot.connected {
            "Connected"
        } else {
            "Disconnected"
        },
    );
    field(
        lines,
        0,
        "Changes",
        if snapshot.changes_saved {
            "Saved"
        } else {
            "Pending"
        },
    );
    field(
        lines,
        0,
        "Sending",
        if snapshot.sending_paused {
            "Paused"
        } else {
            "Active"
        },
    );
    field(lines, 0, "Queued messages", snapshot.queued_messages);
    field(lines, 0, "Steps remaining", snapshot.steps_remaining);
    field(
        lines,
        0,
        "Synchronization waiters",
        snapshot.synchronization_waiters,
    );

    push(
        lines,
        LineStyle::Heading,
        0,
        format!("Blocks ({})", snapshot.blocks.len()),
    );
    if snapshot.blocks.is_empty() {
        empty(lines);
    }
    for block in &snapshot.blocks {
        let name = if block.name.is_empty() {
            "(unnamed)"
        } else {
            &block.name
        };
        push(lines, LineStyle::Body, 1, format!("{name} · {}", block.id));
        field(lines, 2, "Type", block.block_type);
        field(lines, 2, "CRDT", yes_no(block.crdt));
        field(lines, 2, "Ready", yes_no(block.ready));
        field(lines, 2, "Synchronized", yes_no(block.synchronized));
        field(lines, 2, "Local changes", yes_no(block.has_local_changes));
        field(lines, 2, "Confirmed sequence", block.confirmed_seq);
        field(lines, 2, "Acknowledged sequence", block.acknowledged_seq);
        field(lines, 2, "Pending operations", block.pending_operations);
        field(lines, 2, "In-flight operations", block.in_flight_operations);
        field(lines, 2, "Buffered operations", block.buffered_operations);
    }

    push(
        lines,
        LineStyle::Heading,
        0,
        format!("Reference Watches ({})", snapshot.reference_lists.len()),
    );
    if snapshot.reference_lists.is_empty() {
        empty(lines);
    }
    for reference in &snapshot.reference_lists {
        field(lines, 1, "List", format!("{:?}", reference.list));
        field(lines, 2, "Loaded", yes_no(reference.loaded));
        field(lines, 2, "Blocks", reference.blocks);
    }

    push(
        lines,
        LineStyle::Heading,
        0,
        format!("Cache ({})", snapshot.cached_blocks.len()),
    );
    if snapshot.cached_blocks.is_empty() {
        empty(lines);
    }
    for block in &snapshot.cached_blocks {
        let name = properties::read_name(&block.properties)
            .map(|name| name.value)
            .unwrap_or_else(|| "(unnamed)".to_owned());
        push(
            lines,
            LineStyle::Code,
            1,
            format!(
                "{name} · {} · {} · {}",
                block.id, block.block_type, block.author
            ),
        );
    }

    push(
        lines,
        LineStyle::Heading,
        0,
        format!("Pending Requests ({})", snapshot.pending_requests.len()),
    );
    if snapshot.pending_requests.is_empty() {
        empty(lines);
    }
    for request in &snapshot.pending_requests {
        push(
            lines,
            LineStyle::Code,
            1,
            format!(
                "{} {} {}",
                request.request_id, request.kind, request.details
            ),
        );
    }

    entries(lines, "Outbound Queue", &snapshot.outbound_messages);
    entries(lines, "Deferred Queue", &snapshot.deferred_requests);
}

fn entries(lines: &mut Vec<Line>, title: &str, entries: &[ClientDebugEntry]) {
    push(
        lines,
        LineStyle::Heading,
        0,
        format!("{title} ({})", entries.len()),
    );
    if entries.is_empty() {
        empty(lines);
    }
    for (index, entry) in entries.iter().enumerate() {
        push(
            lines,
            LineStyle::Code,
            1,
            format!("{index} {} {}", entry.kind, entry.details),
        );
    }
}

fn be_stack(lines: &mut Vec<Line>, status: &crate::be::Status) {
    push(lines, LineStyle::Heading, 0, "New Block Stack");
    field(
        lines,
        1,
        "Peer",
        if status.running { "Running" } else { "Off" },
    );
    field(
        lines,
        1,
        "Connection",
        if status.connected {
            "Connected"
        } else {
            "Disconnected"
        },
    );
    field(lines, 1, "Live blocks", status.blocks);
    field(lines, 1, "Worker wake-ups", status.wakes);
    field(
        lines,
        1,
        "Changes",
        if status.unsealed == 0 {
            "Sealed".to_owned()
        } else {
            format!("{} block(s) unsealed", status.unsealed)
        },
    );
    field(lines, 1, "Error", status.error.as_deref().unwrap_or("None"));
}
