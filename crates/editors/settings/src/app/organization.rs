use block_editor_beui::be_block::be_model::Document;
use block_editor_beui::be_block::{
    Allowance, BlockContent, BlockMetadata, Edit, Layered, LiveEdit, LostAccess, OfflinePolicy,
    OfflineSettings, OrganizationPolicies, Root, Setting, Settings, SettingsContent,
    WORKSPACE_TYPE, WhoMay,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{BlockParent, BlockQuery, Editor, LayeredSettings};
use uuid::Uuid;

use super::block::name_of;
use super::ui::Section;
use beui::{Extent, ItemSize};

const ABOUT: &str = "Applies to every member. Only its administrators change it, and the server reads it to enforce sharing, workspaces, history and storage.";
const OFFLINE_ABOUT: &str = "Whether this device keeps a workspace's blocks so they open without a connection. When off, this device forgets them when the app closes, except edits that have not reached the server yet.";
const FIXED: &str = "Set by your organization";

const HISTORY_DAYS: [Option<u32>; 5] = [None, Some(7), Some(30), Some(90), Some(365)];
const QUOTAS: [Option<u64>; 4] = [None, Some(10_000), Some(100_000), Some(1_000_000)];
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
        description: "The encrypted objects the server keeps for this organization. Uploads past it are refused.",
        options: &[
            "No limit",
            "10,000 objects",
            "100,000 objects",
            "1,000,000 objects",
        ],
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
    );
    editor
        .content_of::<SettingsContent>(settings)
        .operate(Settings::set_policy(kind, block));
}

#[component]
pub(super) fn OrganizationSection(editor: Editor) -> NodeId {
    let settings = create_memo(move || editor.organization_settings());
    let shown = create_memo(move || settings.get().is_some());
    let manages = create_memo(move || {
        settings
            .get()
            .is_some_and(|settings| editor.blocks().access(settings).can_manage())
    });
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <Section title="Organization" caption={ABOUT}>
                    <Card>
                        <ForEach keys={(0..POLICIES.len()).collect::<Vec<_>>()}>|index: usize| {
                            let row = policy_row::<OrganizationPolicies>(editor, settings, index);
                            view! {
                                <PolicyRow
                                    key={row.key}
                                    label={row.label}
                                    description={row.description}
                                    options={row.options}
                                    selected={row.selected}
                                    choose={row.choose}
                                    locked={create_memo(move || !manages.get())}
                                    separated={row.separated}
                                />
                            }
                        }</ForEach>
                        <ForEach
                            keys={(0..OFFLINE_POLICIES.len()).collect::<Vec<_>>()}
                        >|index: usize| {
                            let row = policy_row::<OfflineSettings>(editor, settings, index);
                            view! {
                                <PolicyRow
                                    key={row.key}
                                    label={row.label}
                                    description={row.description}
                                    options={row.options}
                                    selected={row.selected}
                                    choose={row.choose}
                                    locked={create_memo(move || !manages.get())}
                                    separated={row.separated}
                                />
                            }
                        }</ForEach>
                    </Card>
                </Section>
            </Show>
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

struct Row {
    key: &'static str,
    label: &'static str,
    description: &'static str,
    options: &'static [&'static str],
    selected: Memo<Option<usize>>,
    choose: Callback<Option<usize>>,
    separated: bool,
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
        separated: index > 0 || R::CONTENT_TYPE == OfflineSettings::CONTENT_TYPE,
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
    let numbers: Vec<usize> = (0..options.len()).collect();
    view! {
        <SettingsRow
            label={label.to_owned()}
            description={description.to_owned()}
            separated={separated}
            locked={locked}
        >
            <Select
                @sizing=ItemSize::Percent(100.0)
                width={Extent::fraction(1.0)}
                options={view! {
                    <ForEach keys={numbers}>|option: usize|
                        <ChoiceOption value=option label={options[option]} />
                    </ForEach>
                }}
                selected={Binding::new(selected, move |chosen: Option<usize>| choose.call(chosen))}
                label={label.to_owned()}
                disabled={locked}
                @test_id={format!("settings.organization.{key}")}
            />
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
pub(super) fn OfflineSection(editor: Editor) -> NodeId {
    let roots = editor.watch_blocks(BlockQuery::Roots);
    let workspaces = create_memo(move || {
        let mut found: Vec<Uuid> = roots
            .get()
            .into_iter()
            .flatten()
            .filter(|info| info.block_type == WORKSPACE_TYPE && info.access.can_view())
            .map(|info| info.id)
            .collect();
        found.sort();
        found
    });
    let listed = create_memo(move || !workspaces.with(Vec::is_empty));
    let layered = LayeredSettings::<OfflineSettings>::this_device(editor);
    let resolved = editor.resolved_settings::<OfflineSettings>(editor.block_id());
    view! {
        <List spacing=Space::None>
            <Show condition={listed}>
                <Section title="Offline" caption={OFFLINE_ABOUT}>
                    <Card>
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
                    </Card>
                </Section>
            </Show>
        </List>
    }
}

#[component]
fn Card(children: Children<ListChild>) -> NodeId {
    view! {
        <Frame fill=ColorRole::Surface border={Some(Border::DEFAULT)} radius=Radius::Lg>
            <List spacing=Space::None>{children}</List>
        </Frame>
    }
}
