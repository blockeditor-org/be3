use block::{ClientMessage, ServerMessage};
use block_client::{BlockClient, NetworkDirection, NetworkTrafficEntry};
use serde_json::Value;

use crate::ui::{NetworkView, TrafficRow};

pub(super) fn view(client: &BlockClient) -> NetworkView {
    let debug = client.network_debug_snapshot();
    NetworkView {
        paused: debug.sending_paused,
        queued: debug.queued_messages,
        entries: debug
            .traffic
            .iter()
            .enumerate()
            .map(|(index, entry)| TrafficRow {
                index,
                sent: matches!(entry.direction, NetworkDirection::Sent),
                timestamp: entry.timestamp_ms.to_string(),
                payload: entry.payload.clone(),
                decoded: decoded_payloads(entry),
            })
            .collect(),
    }
}

fn decoded_payloads(entry: &NetworkTrafficEntry) -> Vec<String> {
    match entry.direction {
        NetworkDirection::Sent => serde_json::from_str::<ClientMessage>(&entry.payload)
            .ok()
            .map_or_else(Vec::new, decoded_client_message),
        NetworkDirection::Received => serde_json::from_str::<ServerMessage>(&entry.payload)
            .ok()
            .map_or_else(Vec::new, decoded_server_message),
    }
}

fn decoded_client_message(message: ClientMessage) -> Vec<String> {
    match message {
        ClientMessage::UpdateBlock { id, operation, .. } => {
            decoded_operation(id.to_string(), &operation)
                .into_iter()
                .collect()
        }
        ClientMessage::UpdateBatch { updates, .. } => updates
            .into_iter()
            .filter_map(|update| decoded_operation(update.id.to_string(), &update.operation))
            .collect(),
        ClientMessage::SetPresence {
            id,
            presence_id,
            data: Some(data),
            ..
        } => decoded_presence(id.to_string(), presence_id.to_string(), &data)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn decoded_server_message(message: ServerMessage) -> Vec<String> {
    match message {
        ServerMessage::ReadBlock { id, operations, .. } => operations
            .into_iter()
            .filter_map(|operation| decoded_operation(id.to_string(), &operation.operation))
            .collect(),
        ServerMessage::BatchOk { operations, .. } | ServerMessage::BatchUpdated { operations } => {
            operations
                .into_iter()
                .filter_map(|operation| {
                    decoded_operation(operation.id.to_string(), &operation.operation.operation)
                })
                .collect()
        }
        ServerMessage::BlockUpdated { id, operation, .. } => {
            decoded_operation(id.to_string(), &operation.operation)
                .into_iter()
                .collect()
        }
        ServerMessage::Presence {
            id,
            presence_id,
            data: Some(data),
            ..
        } => decoded_presence(id.to_string(), presence_id.to_string(), &data)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn decoded_operation(id: String, data: &[u8]) -> Option<String> {
    decoded_data(data).map(|data| format!("Operation for block {id}:\n{data}"))
}

fn decoded_presence(id: String, presence_id: String, data: &[u8]) -> Option<String> {
    decoded_data(data).map(|data| format!("Presence {presence_id} for block {id}:\n{data}"))
}

fn decoded_data(data: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(data)
        .ok()
        .and_then(|data| serde_json::to_string_pretty(&data).ok())
}
