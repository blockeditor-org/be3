use be_sdk_beui::be_block::{
    Allowance, LinkSharing, LostAccess, OfflinePolicy, OfflineSettings, OrganizationPolicies,
    Settings, SettingsContent, WhoMay,
};
use be_sdk_beui::be_block_content::be_block_model::Document;
use be_sdk_beui::be_block_content::{
    BlockContent, BlockMetadata, Edit, LiveEdit, Root, Setting,
};
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{BlockParent, Editor, LayeredSettings};
use uuid::Uuid;

use super::block::name_of;
use super::ui::workspaces;
use beui::Extent;

const ABOUT: &str = "Applies to every member. Only its administrators change it, and the server reads it to enforce sharing, workspaces, history and storage.";
const OFFLINE_ABOUT: &str = "Whether this device keeps a workspace's blocks so they open without a connection. When off, this device forgets them when the app closes, except edits that have not reached the server yet.";
const FIXED: &str = "Set by your organization";

const GIGABYTE: u64 = 1_000_000_000;

/// One row of a policy page: a select over the policy's own values, each named beside its
/// label, read from and written to the policy block of kind `R`.
struct Policy<R, V: 'static> {
    key: &'static str,
    label: &'static str,
    description: &'static str,
    options: &'static [(V, &'static str)],
    get: fn(&R) -> V,
    set: fn(V) -> Edit,
}

/// The stored form of a choice whose default is held as no value.
fn unless_default<T: Default + PartialEq>(value: T) -> Option<T> {
    (value != T::default()).then_some(value)
}

const SHARING: Policy<OrganizationPolicies, Allowance> = Policy {
    key: "sharing",
    label: "Sharing outside the organization",
    description: "Blocks are shared only with members. Blocked also refuses links that carry access, since a link reaches whoever holds it.",
    options: &[(Allowance::Allowed, "Allowed"), (Allowance::Blocked, "Blocked")],
    get: |policies| policies.outside_sharing.unwrap_or_default(),
    set: |value| OrganizationPolicies::set_outside_sharing(unless_default(value)),
};

const SHARE_LINKS: Policy<OrganizationPolicies, LinkSharing> = Policy {
    key: "share-links",
    label: "Who share links admit",
    description: "Anyone with a block's link may open it as its maker chose, within this. Anyone with an account joins as a guest, who sees only what links share; it needs sharing outside allowed. Narrowing this leaves grants already made.",
    options: &[
        (LinkSharing::Members, "Members"),
        (LinkSharing::Anyone, "Anyone with an account"),
        (LinkSharing::Off, "No share links"),
    ],
    get: |policies| policies.link_sharing.unwrap_or_default(),
    set: |value| OrganizationPolicies::set_link_sharing(unless_default(value)),
};

const WORKSPACES: Policy<OrganizationPolicies, WhoMay> = Policy {
    key: "workspaces",
    label: "Who makes workspaces",
    description: "Each member still makes one workspace of their own: My files, where the app keeps what they make outside any workspace.",
    options: &[
        (WhoMay::Everyone, "Every member"),
        (WhoMay::Administrators, "Administrators"),
    ],
    get: |policies| policies.workspace_creation(),
    set: |value| OrganizationPolicies::set_workspace_creation(unless_default(value)),
};

const HISTORY: Policy<OrganizationPolicies, Option<u32>> = Policy {
    key: "history",
    label: "Keep every version for",
    description: "Nobody's pruning removes a version younger than this.",
    options: &[
        (None, "As each block's history thins"),
        (Some(7), "7 days"),
        (Some(30), "30 days"),
        (Some(90), "90 days"),
        (Some(365), "A year"),
    ],
    get: |policies| policies.history_days,
    set: OrganizationPolicies::set_history_days,
};

const QUOTA: Policy<OrganizationPolicies, Option<u64>> = Policy {
    key: "quota",
    label: "Storage quota",
    description: "The bytes of encrypted objects the server keeps for this organization. Uploads past it are refused.",
    options: &[
        (None, "No limit"),
        (Some(10 * GIGABYTE), "10 GB"),
        (Some(100 * GIGABYTE), "100 GB"),
        (Some(1_000 * GIGABYTE), "1 TB"),
    ],
    get: |policies| policies.storage_quota,
    set: OrganizationPolicies::set_storage_quota,
};

const OFFLINE: Policy<OfflineSettings, OfflinePolicy> = Policy {
    key: "offline",
    label: "Keeping workspaces offline",
    description: "Whether members' devices keep workspaces so they open without a connection.",
    options: &[
        (OfflinePolicy::Allowed, "Each member chooses"),
        (OfflinePolicy::Required, "Required"),
        (OfflinePolicy::Blocked, "Blocked"),
    ],
    get: |offline| offline.policy.unwrap_or_default(),
    set: |value| OfflineSettings::set_policy(unless_default(value)),
};

const LOST_ACCESS: Policy<OfflineSettings, LostAccess> = Policy {
    key: "lost-access",
    label: "When a member loses access",
    description: "What their devices do with what they kept. Personal accounts never delete anything by themselves.",
    options: &[(LostAccess::Keep, "Keep it"), (LostAccess::Delete, "Delete it")],
    get: |offline| offline.on_lost_access.unwrap_or_default(),
    set: |value| OfflineSettings::set_on_lost_access(unless_default(value)),
};

const GRACE: Policy<OfflineSettings, Option<u32>> = Policy {
    key: "grace",
    label: "Unsent edits wait for",
    description: "How long edits that could not reach the server wait before they are deleted.",
    options: &[(Some(1), "1 day"), (None, "7 days"), (Some(30), "30 days")],
    get: |offline| offline.grace_days,
    set: OfflineSettings::set_grace_days,
};

fn policy_block(editor: Editor, settings: Uuid, kind: Uuid) -> Option<Uuid> {
    editor
        .content_of::<SettingsContent>(settings)
        .read(|content| content.root().policy(kind))
        .ok()
        .flatten()
}

fn write_policy<R>(editor: Editor, settings: Uuid, edit: Edit)
where
    R: Root + Default + 'static,
    Document<R>: BlockContent + LiveEdit<Op = Edit> + Clone + Default,
{
    let kind = R::CONTENT_TYPE;
    if let Some(block) = untrack(|| policy_block(editor, settings, kind)) {
        editor.content_of::<Document<R>>(block).operate(edit);
        return;
    }
    let mut content = Document::<R>::default();
    content.apply(&edit);
    let block = editor.blocks().create_at(
        Uuid::now_v7(),
        kind,
        Some(content.encode()),
        BlockParent::Block(settings),
        BlockMetadata::default(),
        false,
    );
    editor
        .content_of::<SettingsContent>(settings)
        .operate(Settings::set_policy(kind, block));
}

#[component]
pub(super) fn OrganizationPage(editor: Editor) -> NodeId {
    let block = editor.block_id();
    let settings = create_memo(move || Some(block));
    let locked = create_memo(move || !editor.blocks().access(block).can_manage());
    view! {
        <SettingsPage>
            <SettingsSection title="Sharing and workspaces" description={ABOUT}>
                <List spacing=Space::None>
                    {policy_row(editor, settings, &SHARING, locked, false)}
                    {policy_row(editor, settings, &SHARE_LINKS, locked, true)}
                    {policy_row(editor, settings, &WORKSPACES, locked, true)}
                </List>
            </SettingsSection>
            <SettingsSection title="History and storage">
                <List spacing=Space::None>
                    {policy_row(editor, settings, &HISTORY, locked, false)}
                    {policy_row(editor, settings, &QUOTA, locked, true)}
                </List>
            </SettingsSection>
            <SettingsSection title="Devices">
                <List spacing=Space::None>
                    {policy_row(editor, settings, &OFFLINE, locked, false)}
                    {policy_row(editor, settings, &LOST_ACCESS, locked, true)}
                    {policy_row(editor, settings, &GRACE, locked, true)}
                </List>
            </SettingsSection>
        </SettingsPage>
    }
}

/// One policy's row, given whether the page is locked and whether a row is above it.
fn policy_row<R, V>(
    editor: Editor,
    settings: Memo<Option<Uuid>>,
    policy: &'static Policy<R, V>,
    locked: Memo<bool>,
    separated: bool,
) -> NodeId
where
    R: Root + Default + Clone + 'static,
    Document<R>: BlockContent + LiveEdit<Op = Edit> + Clone + Default,
    V: Copy + PartialEq + 'static,
{
    let selected = create_memo(move || {
        let settings = settings.get()?;
        let held = policy_block(editor, settings, R::CONTENT_TYPE)
            .and_then(|block| {
                editor
                    .content_of::<Document<R>>(block)
                    .read(|content| content.root())
                    .ok()
            })
            .unwrap_or_default();
        let value = (policy.get)(&held);
        // A value none of the options is shows as no choice.
        policy
            .options
            .iter()
            .any(|(option, _)| *option == value)
            .then_some(value)
    });
    let choose = move |chosen: Option<V>| {
        if let (Some(chosen), Some(settings)) = (chosen, settings.get_untracked()) {
            write_policy::<R>(editor, settings, (policy.set)(chosen));
        }
    };
    let options: Vec<usize> = (0..policy.options.len()).collect();
    view! {
        <SettingsRow
            label={policy.label.to_owned()}
            description={policy.description.to_owned()}
            separated={separated}
            locked={locked}
        >
            <ListItem grow=1.0>
                <Select
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach value={options}>|option: usize| {
                            let (value, label) = policy.options[option];
                            view! { <ChoiceOption value=value label={label.to_owned()} /> }
                        }</ForEach>
                    }}
                    selected={Binding::new(selected, choose)}
                    label={policy.label.to_owned()}
                    disabled={locked}
                    @test_id={format!("settings.organization.{}", policy.key)}
                />
            </ListItem>
        </SettingsRow>
    }
}

fn keep_setting(workspace: Uuid) -> Setting<OfflineSettings, bool> {
    Setting::new(
        format!("offline.{workspace}"),
        "Keep for offline use",
        move |offline: &OfflineSettings| offline.workspaces.get(&workspace).copied(),
        move |_, keep| OfflineSettings::keep_workspace(workspace, keep),
    )
    .only_on_this_device(true)
}

#[component]
pub(super) fn OfflinePage(editor: Editor) -> NodeId {
    let workspaces = workspaces(editor);
    let layered = LayeredSettings::<OfflineSettings>::this_device(editor);
    let resolved = editor.resolved_settings::<OfflineSettings>(editor.block_id());
    view! {
        <SettingsPage>
            <SettingsSection title="Offline" description={OFFLINE_ABOUT}>
                <ForEach
                    value={workspaces}
                    fallback={|| view! {
                        <SettingsRow label="No workspaces yet" compact=true />
                    }}
                >|workspace: Uuid, index: ReadSignal<usize>| {
                    let setting = keep_setting(workspace);
                    let personal = create_memo(move || editor.organization_settings().is_none());
                    let choice = create_memo(move || {
                        resolved.with(|resolved| {
                            OfflineSettings::choice(resolved, workspace, personal.get())
                        })
                    });
                    let on = create_memo(move || choice.get().keep);
                    let fixed = create_memo(move || choice.get().fixed);
                    view! {
                        <SettingsRow
                            label={move || name_of(editor, workspace)}
                            note={move || match fixed.get() {
                                true => FIXED.to_owned(),
                                false => String::new(),
                            }}
                            separated={move || index.get() != 0}
                            locked={fixed}
                            compact=true
                        >
                            <Switch
                                on={Binding::new(on, move |keep: bool| layered.set(&setting, Some(keep)))}
                                label="Keep for offline use"
                                disabled={fixed}
                                @test_id={format!("settings.offline.{workspace}")}
                            />
                        </SettingsRow>
                    }
                }</ForEach>
            </SettingsSection>
        </SettingsPage>
    }
}
