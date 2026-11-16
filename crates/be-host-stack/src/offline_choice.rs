use std::collections::HashMap;
use std::sync::PoisonError;

use be_wasm_threads::Mutex;

use be_block::{OfflineChoice, OfflineSettings, Resolution};
use uuid::Uuid;

/// What the settings choose to keep offline. The page works it out (`publish`) and sends it to
/// the stack's worker, which reads it (`choice`).
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Choices {
    workspaces: HashMap<Uuid, OfflineChoice>,
    otherwise: OfflineChoice,
}

static CHOICES: Mutex<Option<Choices>> = Mutex::new(None);

pub fn choice(workspace: Uuid) -> OfflineChoice {
    let held = CHOICES.lock().unwrap_or_else(PoisonError::into_inner);
    match held.as_ref() {
        Some(choices) => choices
            .workspaces
            .get(&workspace)
            .copied()
            .unwrap_or(choices.otherwise),
        None => OfflineSettings::choice(&Default::default(), workspace, true),
    }
}

pub fn publish(resolution: &Resolution<OfflineSettings>, workspaces: Vec<Uuid>, personal: bool) {
    let choices = Choices {
        workspaces: workspaces
            .into_iter()
            .map(|workspace| {
                (
                    workspace,
                    OfflineSettings::choice(resolution, workspace, personal),
                )
            })
            .collect(),
        otherwise: OfflineSettings::choice(resolution, Uuid::nil(), personal),
    };
    tell(Some(choices));
}

pub fn forget() {
    tell(None);
}

/// Keeps `choices` here and, when they changed, sends them to the stack's worker, which keeps
/// its own copy (`set`).
fn tell(choices: Option<Choices>) {
    let mut held = CHOICES.lock().unwrap_or_else(PoisonError::into_inner);
    if *held == choices {
        return;
    }
    *held = choices.clone();
    drop(held);
    crate::send(crate::worker::Command::OfflineChoices(choices));
}

/// What `publish` chose last, for a stack that starts.
pub(crate) fn current() -> Option<Choices> {
    CHOICES.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// In the stack's worker: keeps what the page chose (`tell`).
pub(crate) fn set(choices: Option<Choices>) {
    *CHOICES.lock().unwrap_or_else(PoisonError::into_inner) = choices;
}
