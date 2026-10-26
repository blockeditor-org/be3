use std::collections::HashMap;
use std::sync::PoisonError;

use wasi_threads::Mutex;

use be_block::{OfflineChoice, OfflineSettings};
use uuid::Uuid;

use crate::root_settings::RootSettings;

#[derive(Clone, Debug, PartialEq)]
struct Choices {
    workspaces: HashMap<Uuid, OfflineChoice>,
    otherwise: OfflineChoice,
}

static CHOICES: Mutex<Option<Choices>> = Mutex::new(None);

pub(crate) fn choice(workspace: Uuid) -> OfflineChoice {
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

pub(crate) fn publish(root: &RootSettings, client: Uuid, organization: Option<(Uuid, bool)>) {
    let company = organization.and_then(|(id, company)| company.then_some(id));
    let personal = company.is_none();
    let resolution = root.offline_settings(client, company);
    let choices = Choices {
        workspaces: root
            .workspaces()
            .into_iter()
            .map(|workspace| {
                (
                    workspace,
                    OfflineSettings::choice(&resolution, workspace, personal),
                )
            })
            .collect(),
        otherwise: OfflineSettings::choice(&resolution, Uuid::nil(), personal),
    };
    let mut held = CHOICES.lock().unwrap_or_else(PoisonError::into_inner);
    if held.as_ref() != Some(&choices) {
        *held = Some(choices);
    }
}

pub(crate) fn forget() {
    *CHOICES.lock().unwrap_or_else(PoisonError::into_inner) = None;
}
