use std::collections::HashMap;
use std::sync::PoisonError;

use wasi_threads::Mutex;

use be_block::{OfflineChoice, OfflineSettings, Resolution};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
struct Choices {
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
    let mut held = CHOICES.lock().unwrap_or_else(PoisonError::into_inner);
    if held.as_ref() != Some(&choices) {
        *held = Some(choices);
    }
}

pub fn forget() {
    *CHOICES.lock().unwrap_or_else(PoisonError::into_inner) = None;
}
