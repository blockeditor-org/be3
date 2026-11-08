use be_sdk_beui::be_block::be_block_model::Document;
use be_sdk_beui::be_block::{
    Allowance, BlockContent, BlockMetadata, Edit, Layered, LiveEdit, LostAccess, OfflinePolicy,
    OfflineSettings, OrganizationPolicies, Root, Setting, Settings, SettingsContent, WhoMay,
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

const HISTORY_DAYS: [Option<u32>; 5] = [None, Some(7), Some(30), Some(90), Some(365)];
const GIGABYTE: u64 = 1_000_000_000;
const QUOTAS: [Option<u64>; 4] = [
    None,
    Some(10 * GIGABYTE),
    Some(100 * GIGABYTE),
    Some(1_000 * GIGABYTE),
];
const GRACE_DAYS: [Option<u32>; 3] = [Some(1), None, Some(30)];

struct Policy<R> {
    key: &'static str,
    label: &'static str,
    description: &'static str,
    options: &'static [&'static str],
    get: fn(&R) -> usize,
    set: fn(usize) -> Edit,
}

fn index_of<T: PartialEq>(all: &[T], value: &T) -> usize {
    all.iter().position(|held| held == value).unwrap_or(0)
}

const POLICIES: [Policy<OrganizationPolicies>; 4] = [
    Policy {
        key: "sharing",
        label: "Sharing outside the organization",
        description: "Blocks are shared only with members. Blocked also refuses links that carry access, since a link reaches whoever holds it.",
        options: &["Allowed", "Blocked"],
        get: |policies| {
            index_of(
                &[Allowance::Allowed, Allowance::Blocked],
                &policies.outside_sharing.unwrap_or_default(),
            )
        },
        set: |index| {
            OrganizationPolicies::set_outside_sharing(
                [None, Some(Allowance::Blocked)][index.min(1)],
            )
        },
    },
    Policy {
        key: "workspaces",
        label: "Who makes workspaces",
        description: "Each member still makes one workspace of their own: My files, where the app keeps what they make outside any workspace.",
        options: &["Every member", "Administrators"],
        get: |policies| {
            index_of(
                &[WhoMay::Everyone, WhoMay::Administrators],
                &policies.workspace_creation(),
            )
        },
        set: |index| {
            OrganizationPolicies::set_workspace_creation(
                [None, Some(WhoMay::Administrators)][index.min(1)],
            )
        },
    },
    Policy {
        key: "history",
        label: "Keep every version for",
        description: "Nobody's pruning removes a version younger than this.",
        options: &[
            "As each block's history thins",
            "7 days",
            "30 days",
            "90 days",
            "A year",
        ],
        get: |policies| index_of(&HISTORY_DAYS, &policies.history_days),
        set: |index| {
            OrganizationPolicies::set_history_days(HISTORY_DAYS[index.min(HISTORY_DAYS.len() - 1)])
        },
    },
    Policy {
        key: "quota",
        label: "Storage quota",
        description: "The bytes of encrypted objects the server keeps for this organization. Uploads past it are refused.",
        options: &["No limit", "10 GB", "100 GB", "1 TB"],
        get: |policies| index_of(&QUOTAS, &policies.storage_quota),
        set: |index| OrganizationPolicies::set_storage_quota(QUOTAS[index.min(QUOTAS.len() - 1)]),
    },
];

const OFFLINE_POLICIES: [Policy<OfflineSettings>; 3] = [
    Policy {
        key: "offline",
        label: "Keeping workspaces offline",
        description: "Whether members' devices keep workspaces so they open without a connection.",
        options: &["Each member chooses", "Required", "Blocked"],
        get: |offline| {
            index_of(
                &[
                    OfflinePolicy::Allowed,
                    OfflinePolicy::Required,
                    OfflinePolicy::Blocked,
                ],
                &offline.policy.unwrap_or_default(),
            )
        },
        set: |index| {
            OfflineSettings::set_policy(
                [
                    None,
                    Some(OfflinePolicy::Required),
                    Some(OfflinePolicy::Blocked),
                ][index.min(2)],
            )
        },
    },
    Policy {
        key: "lost-access",
        label: "When a member loses access",
        description: "What their devices do with what they kept. Personal accounts never delete anything by themselves.",
        options: &["Keep it", "Delete it"],
        get: |offline| {
            index_of(
                &[LostAccess::Keep, LostAccess::Delete],
                &offline.on_lost_access.unwrap_or_default(),
            )
        },
        set: |index| {
            OfflineSettings::set_on_lost_access([None, Some(LostAccess::Delete)][index.min(1)])
        },
    },
    Policy {
        key: "grace",
        label: "Unsent edits wait for",
        description: "How long edits that could not reach the server wait before they are deleted.",
        options: &["1 day", "7 days", "30 days"],
        get: |offline| index_of(&GRACE_DAYS, &offline.grace_days),
        set: |index| OfflineSettings::set_grace_days(GRACE_DAYS[index.min(GRACE_DAYS.len() - 1)]),
    },
];

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
    let organization = |indices: &[usize]| -> Vec<Row> {
        indices
            .iter()
            .map(|index| policy_row::<OrganizationPolicies>(editor, settings, *index))
            .collect()
    };
    let sharing = organization(&[0, 1]);
    let storage = organization(&[2, 3]);
    let devices: Vec<Row> = (0..OFFLINE_POLICIES.len())
        .map(|index| policy_row::<OfflineSettings>(editor, settings, index))
        .collect();
    view! {
        <SettingsPage>
            <SettingsSection title="Sharing and workspaces" description={ABOUT}>
                <PolicyRows rows={sharing} locked={locked} />
            </SettingsSection>
            <SettingsSection title="History and storage">
                <PolicyRows rows={storage} locked={locked} />
            </SettingsSection>
            <SettingsSection title="Devices">
                <PolicyRows rows={devices} locked={locked} />
            </SettingsSection>
        </SettingsPage>
    }
}

#[component]
fn PolicyRows(rows: Vec<Row>, locked: Memo<bool>) -> NodeId {
    let rows = std::rc::Rc::new(rows);
    let keys: Vec<usize> = (0..rows.len()).collect();
    view! {
        <List spacing=Space::None>
            <ForEach keys={keys}>|index: usize| {
                let row = rows[index].clone();
                view! {
                    <PolicyRow
                        key={row.key}
                        label={row.label}
                        description={row.description}
                        options={row.options}
                        selected={row.selected}
                        choose={row.choose}
                        locked={locked}
                        separated={index > 0}
                    />
                }
            }</ForEach>
        </List>
    }
}

trait Policies: Layered + PartialEq + 'static {
    fn policies() -> &'static [Policy<Self>];
}

impl Policies for OrganizationPolicies {
    fn policies() -> &'static [Policy<Self>] {
        &POLICIES
    }
}

impl Policies for OfflineSettings {
    fn policies() -> &'static [Policy<Self>] {
        &OFFLINE_POLICIES
    }
}

#[derive(Clone)]
struct Row {
    key: &'static str,
    label: &'static str,
    description: &'static str,
    options: &'static [&'static str],
    selected: Memo<Option<usize>>,
    choose: Callback<Option<usize>>,
}

fn policy_row<R: Policies>(editor: Editor, settings: Memo<Option<Uuid>>, index: usize) -> Row
where
    Document<R>: BlockContent + LiveEdit<Op = Edit> + Clone + Default,
{
    let policy = &R::policies()[index];
    let (get, set) = (policy.get, policy.set);
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
        Some(get(&held))
    });
    let choose = Callback::new(move |chosen: Option<usize>| {
        if let (Some(chosen), Some(settings)) = (chosen, settings.get_untracked()) {
            write_policy::<R>(editor, settings, set(chosen));
        }
    });
    Row {
        key: policy.key,
        label: policy.label,
        description: policy.description,
        options: policy.options,
        selected,
        choose,
    }
}

#[component]
fn PolicyRow(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    options: &'static [&'static str],
    selected: Memo<Option<usize>>,
    choose: Callback<Option<usize>>,
    locked: Memo<bool>,
    separated: bool,
) -> NodeId {
    let options: Vec<String> = options.iter().map(|label| (*label).to_owned()).collect();
    view! {
        <SettingsRow
            label={label.to_owned()}
            description={description.to_owned()}
            separated={separated}
            locked={locked}
        >
            <ListItem grow=1.0>
                <Select
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={options}>|label: String|
                            <ChoiceOption label={label} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected, move |chosen: Option<usize>| choose.call(chosen))}
                    label={label.to_owned()}
                    disabled={locked}
                    @test_id={format!("settings.organization.{key}")}
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
    let listed = create_memo(move || !workspaces.with(Vec::is_empty));
    let none = create_memo(move || !listed.get());
    let layered = LayeredSettings::<OfflineSettings>::this_device(editor);
    let resolved = editor.resolved_settings::<OfflineSettings>(editor.block_id());
    view! {
        <SettingsPage>
            <SettingsSection title="Offline" description={OFFLINE_ABOUT}>
                <Show condition={none}>
                    <SettingsRow label="No workspaces yet" compact=true />
                </Show>
                <ForEach keys={workspaces}>|workspace: Uuid| {
                    let setting = keep_setting(workspace);
                    let personal = create_memo(move || editor.organization_settings().is_none());
                    let choice = create_memo(move || {
                        resolved.with(|resolved| {
                            OfflineSettings::choice(resolved, workspace, personal.get())
                        })
                    });
                    let on = create_memo(move || choice.get().keep);
                    let fixed = create_memo(move || choice.get().fixed);
                    let note = create_memo(move || match fixed.get() {
                        true => FIXED.to_owned(),
                        false => String::new(),
                    });
                    let label = create_memo(move || name_of(editor, workspace));
                    let first = create_memo(move || workspaces.with(|all| all.first() != Some(&workspace)));
                    view! {
                        <SettingsRow
                            label={label}
                            note={note}
                            separated={first}
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
