use super::canvas::ScaleDirection;
use super::*;
use block_editor_plugin::ContentProjection;
use block_editor_plugin::be_block::Item;
use block_editor_plugin::be_block::hotbar::{
    HotbarContent, HotbarSlot as BlockHotbarSlot, SlotKind,
};

pub(super) fn default_hotbar() -> Vec<HotbarSlot> {
    vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Builtin(ToolKind::MergerSplitter),
        HotbarSlot::Folder {
            name: "Component".to_string(),
            slots: default_component_slots(),
        },
        HotbarSlot::Folder {
            name: "Logic".to_string(),
            slots: vec![HotbarSlot::Builtin(ToolKind::Not)],
        },
        HotbarSlot::Folder {
            name: "Storage".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Storage),
                HotbarSlot::Builtin(ToolKind::ConfigureStorage),
                HotbarSlot::Locked {
                    name: "Register".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Memory".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Display".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Led),
                HotbarSlot::Locked {
                    name: "Seven Segment".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Organization".to_string(),
            slots: vec![
                HotbarSlot::Locked {
                    name: "Comment".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Pattern".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Group".to_string(),
                },
            ],
        },
    ]
}

pub(super) fn default_component_slots() -> Vec<HotbarSlot> {
    vec![
        HotbarSlot::Builtin(ToolKind::Input),
        HotbarSlot::Builtin(ToolKind::Output),
    ]
}

impl LogicGridEditor {
    pub(super) fn select_tool(&mut self) {
        self.tool.kind = ToolKind::Select;
        self.active_hotbar_slot = None;
        self.gesture = None;
        self.configured_storage = None;
    }

    pub(super) fn select_hotbar_path(&mut self, path: Vec<usize>) {
        let Some(slot) = get_hotbar_slot(&self.hotbar, &path).cloned() else {
            return;
        };
        match slot {
            HotbarSlot::Builtin(kind) => {
                self.close_hotbar_folder_if_selecting_outside(&path);
                self.tool.kind = kind;
                self.active_hotbar_slot = Some(path);
                self.gesture = None;
                self.configured_storage = None;
                self.selection.clear();
            }
            HotbarSlot::Component { .. } => {
                self.select_custom_hotbar_path(path);
            }
            HotbarSlot::Folder { .. } => {
                self.active_hotbar_folder = path;
            }
            HotbarSlot::Locked { .. } => {}
        }
    }

    fn select_custom_hotbar_path(&mut self, path: Vec<usize>) {
        self.close_hotbar_folder_if_selecting_outside(&path);
        self.tool.kind = ToolKind::Custom;
        self.active_hotbar_slot = Some(path);
        self.gesture = None;
        self.configured_storage = None;
        self.selection.clear();
    }

    fn close_hotbar_folder_if_selecting_outside(&mut self, path: &[usize]) {
        if !self.active_hotbar_folder.is_empty() && !path.starts_with(&self.active_hotbar_folder) {
            self.active_hotbar_folder.clear();
        }
    }

    pub(super) fn sync_hotbar(&mut self, client: Option<&BlockClient>, client_id: Uuid) -> bool {
        if let Some(client) = client {
            let pending = self.hotbar_needs_write;
            let root = self
                .hotbar_block
                .get_or_insert_with(|| RootSetting::new(client));
            if pending {
                root.ensure(client, client_id);
            } else {
                root.find(client, client_id);
            }
        }
        if self.hotbar_needs_write {
            self.persist_hotbar();
        }

        let Some(slots) = self
            .hotbar_content()
            .and_then(|content| content.read(|hotbar| hotbar.root().slots.to_vec()))
        else {
            return false;
        };
        if client.is_some() {
            for compiled in pinned_components(&slots) {
                self.ensure_compiled(compiled);
            }
        }
        let rebuilt = if slots.is_empty() {
            default_hotbar()
        } else {
            slots
                .iter()
                .filter_map(|slot| self.hotbar_slot_from_block(slot))
                .collect()
        };
        if hotbar_slots_equal(&self.hotbar, &rebuilt) {
            return false;
        }
        self.hotbar = rebuilt;
        if self.tool.kind == ToolKind::Custom {
            self.tool.kind = ToolKind::Select;
            self.active_hotbar_slot = None;
        }
        self.active_hotbar_folder.clear();
        true
    }

    fn hotbar_content(&self) -> Option<Rc<ContentProjection<HotbarContent>>> {
        let id = self.hotbar_block.as_ref()?.block()?.id();
        Some(self.store.editor()?.content_of::<HotbarContent>(id))
    }

    fn hotbar_slot_from_block(&self, slot: &Item<BlockHotbarSlot>) -> Option<HotbarSlot> {
        let slots = &slot.slots;
        Some(match &slot.kind {
            SlotKind::Builtin { tool } => ToolKind::from_id(tool).map_or_else(
                || HotbarSlot::Locked { name: tool.clone() },
                HotbarSlot::Builtin,
            ),
            SlotKind::Locked { name } => HotbarSlot::Locked { name: name.clone() },
            SlotKind::Folder { name } => HotbarSlot::Folder {
                name: name.clone(),
                slots: slots
                    .iter()
                    .filter_map(|slot| self.hotbar_slot_from_block(slot))
                    .collect(),
            },
            SlotKind::Component { name, compiled } => {
                let compiled = compiled.as_direct()?;
                HotbarSlot::Component {
                    name: name.clone(),
                    compiled,
                    kind: self.compiled_kind(compiled, name),
                }
            }
        })
    }

    pub(super) fn pin_component(&mut self, name: String, compiled: Uuid) {
        let kind = self.compiled_kind(compiled, &name);
        let slot = HotbarSlot::Component {
            name,
            compiled,
            kind,
        };
        if let Some(existing) = find_custom_hotbar_slot_mut(&mut self.hotbar, compiled) {
            *existing = slot;
        } else {
            self.hotbar.push(slot);
        }
        self.persist_hotbar();
    }

    pub(super) fn remove_hotbar_slot(&mut self, path: &[usize]) {
        if !matches!(
            get_hotbar_slot(&self.hotbar, path),
            Some(HotbarSlot::Component { .. })
        ) {
            return;
        }
        remove_hotbar_slot_at(&mut self.hotbar, path);
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(path))
        {
            self.active_hotbar_slot = None;
            if self.tool.kind == ToolKind::Custom {
                self.select_tool();
            }
        }
    }

    pub(super) fn remove_hotbar_folder(&mut self, path: &[usize]) {
        if !matches!(
            get_hotbar_slot(&self.hotbar, path),
            Some(HotbarSlot::Folder { .. })
        ) || hotbar_slot_contains_unremovable(get_hotbar_slot(&self.hotbar, path).unwrap())
        {
            return;
        }
        remove_hotbar_slot_at(&mut self.hotbar, path);
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(path))
        {
            self.select_tool();
        }
        if self.active_hotbar_folder.starts_with(path) {
            self.active_hotbar_folder.clear();
        }
    }

    pub(super) fn reset_hotbar(&mut self) {
        self.hotbar = default_hotbar();
        self.active_hotbar_folder.clear();
        self.select_tool();
        self.persist_hotbar();
    }

    pub(super) fn persist_hotbar(&mut self) {
        let slots: Vec<BlockHotbarSlot> = self.hotbar.iter().map(hotbar_slot_to_block).collect();
        let Some((content, edit)) = self.hotbar_content().and_then(|content| {
            let edit = content.read(|hotbar| hotbar.root().replace_all(slots))?;
            Some((content, edit))
        }) else {
            self.hotbar_needs_write = true;
            return;
        };
        content.operate(edit);
        self.hotbar_needs_write = false;
    }

    pub(super) fn hotbar_key_entries(&self) -> Vec<(Vec<usize>, HotbarSlot)> {
        visible_hotbar_rows(&self.hotbar, &self.active_hotbar_folder)
            .into_iter()
            .rev()
            .find(|row| !row.entries.is_empty())
            .map(|row| row.entries)
            .unwrap_or_default()
    }

    pub(super) fn hotbar_key(&mut self, key: Key) -> bool {
        if let Some(index) = HOTBAR_KEYS.iter().position(|hotkey| *hotkey == key) {
            if let Some((path, slot)) = self.hotbar_key_entries().get(index)
                && !self.hotbar_slot_disabled(slot)
            {
                self.click_hotbar_slot(path.clone());
            }
            return true;
        }
        match key {
            Key::Backtick => {
                self.active_hotbar_folder.pop();
            }
            Key::BracketLeft => step_scale(&mut self.tool.scale, ScaleDirection::Down),
            Key::BracketRight => step_scale(&mut self.tool.scale, ScaleDirection::Up),
            _ => return false,
        }
        true
    }

    pub(super) fn click_hotbar_slot(&mut self, path: Vec<usize>) {
        if self.active_hotbar_slot.as_ref() == Some(&path) {
            self.select_tool();
        } else {
            self.select_hotbar_path(path);
        }
    }

    pub(super) fn toggle_hotbar_folder(&mut self, path: Vec<usize>) {
        if self.active_hotbar_folder == path {
            self.active_hotbar_folder.pop();
        } else {
            self.active_hotbar_folder = path;
        }
    }

    pub(super) fn new_hotbar_folder(&mut self) {
        let slots = get_hotbar_slots_mut(&mut self.hotbar, &self.active_hotbar_folder);
        slots.push(HotbarSlot::Folder {
            name: "Folder".to_string(),
            slots: Vec::new(),
        });
        self.persist_hotbar();
    }

    pub(super) fn drop_hotbar_slot(&mut self, target: Option<HotbarDropTarget>) {
        let (Some(source), Some(target)) = (self.hotbar_drag.take(), target) else {
            return;
        };
        let placed = match target {
            HotbarDropTarget::Slot(target) => move_hotbar_slot(&mut self.hotbar, &source, &target),
            HotbarDropTarget::Folder(target) => {
                move_hotbar_slot_to_folder(&mut self.hotbar, &source, &target)
            }
        };
        let Some(placed) = placed else {
            return;
        };
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(&source))
        {
            self.select_tool();
        }
        if self.active_hotbar_folder.starts_with(&source) {
            self.active_hotbar_folder.clear();
        }
        self.active_hotbar_folder = followed(&self.active_hotbar_folder, &source, &placed);
        self.active_hotbar_slot = self
            .active_hotbar_slot
            .as_deref()
            .map(|path| followed(path, &source, &placed));
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum HotbarDropTarget {
    Slot(Vec<usize>),
    Folder(Vec<usize>),
}

pub(super) fn hotbar_key_label(index: usize) -> Option<&'static str> {
    match index {
        0 => Some("1"),
        1 => Some("2"),
        2 => Some("3"),
        3 => Some("4"),
        4 => Some("5"),
        5 => Some("6"),
        6 => Some("7"),
        7 => Some("8"),
        8 => Some("9"),
        9 => Some("0"),
        _ => None,
    }
}

pub(super) fn hotbar_slot_to_block(slot: &HotbarSlot) -> BlockHotbarSlot {
    let leaf = |kind| BlockHotbarSlot {
        kind,
        slots: Default::default(),
    };
    match slot {
        HotbarSlot::Builtin(kind) => leaf(SlotKind::Builtin {
            tool: kind.id().to_string(),
        }),
        HotbarSlot::Locked { name } => leaf(SlotKind::Locked { name: name.clone() }),
        HotbarSlot::Folder { name, slots } => {
            BlockHotbarSlot::folder(name.clone(), slots.iter().map(hotbar_slot_to_block))
        }
        HotbarSlot::Component { name, compiled, .. } => {
            BlockHotbarSlot::component(name.clone(), BlockRef::Direct(*compiled))
        }
    }
}

fn pinned_components(slots: &[Item<BlockHotbarSlot>]) -> Vec<Uuid> {
    let mut pinned = Vec::new();
    for slot in slots {
        pinned.extend(slot.compiled().and_then(|compiled| compiled.as_direct()));
        pinned.extend(pinned_components(&slot.slots));
    }
    pinned
}

fn hotbar_slots_equal(left: &[HotbarSlot], right: &[HotbarSlot]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (HotbarSlot::Builtin(left), HotbarSlot::Builtin(right)) => left == right,
                (HotbarSlot::Locked { name: left }, HotbarSlot::Locked { name: right }) => {
                    left == right
                }
                (
                    HotbarSlot::Folder {
                        name: left_name,
                        slots: left_slots,
                    },
                    HotbarSlot::Folder {
                        name: right_name,
                        slots: right_slots,
                    },
                ) => left_name == right_name && hotbar_slots_equal(left_slots, right_slots),
                (
                    HotbarSlot::Component {
                        name: left_name,
                        compiled: left_compiled,
                        kind: left_kind,
                    },
                    HotbarSlot::Component {
                        name: right_name,
                        compiled: right_compiled,
                        kind: right_kind,
                    },
                ) => {
                    left_name == right_name
                        && left_compiled == right_compiled
                        && left_kind == right_kind
                }
                _ => false,
            })
}

pub(super) fn hotbar_slot_contains_unremovable(slot: &HotbarSlot) -> bool {
    match slot {
        HotbarSlot::Builtin(_) | HotbarSlot::Locked { .. } => true,
        HotbarSlot::Folder { slots, .. } => slots.iter().any(hotbar_slot_contains_unremovable),
        HotbarSlot::Component { .. } => false,
    }
}

pub(super) struct HotbarRow {
    pub(super) folder_path: Vec<usize>,
    pub(super) title: String,
    pub(super) active: bool,
    pub(super) entries: Vec<(Vec<usize>, HotbarSlot)>,
}

pub(super) fn visible_hotbar_rows(slots: &[HotbarSlot], active_folder: &[usize]) -> [HotbarRow; 2] {
    let (first_path, second_path) = if active_folder.len() <= 1 {
        (
            Vec::new(),
            (!active_folder.is_empty()).then(|| active_folder.to_vec()),
        )
    } else {
        (
            active_folder[..active_folder.len() - 1].to_vec(),
            Some(active_folder.to_vec()),
        )
    };
    let first_active = second_path.is_none();
    [
        hotbar_row(slots, first_path, first_active),
        match second_path {
            Some(path) => hotbar_row(slots, path, true),
            None => HotbarRow {
                folder_path: Vec::new(),
                title: "Open folder".to_string(),
                active: false,
                entries: Vec::new(),
            },
        },
    ]
}

pub(super) fn hotbar_row(slots: &[HotbarSlot], folder_path: Vec<usize>, active: bool) -> HotbarRow {
    let title = hotbar_folder_name(slots, &folder_path).to_string();
    let entries = visible_hotbar_entries(slots, &folder_path);
    HotbarRow {
        folder_path,
        title,
        active,
        entries,
    }
}

pub(super) fn visible_hotbar_entries(
    slots: &[HotbarSlot],
    folder_path: &[usize],
) -> Vec<(Vec<usize>, HotbarSlot)> {
    let slots = get_hotbar_slots(slots, folder_path).unwrap_or(slots);
    slots
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, slot)| {
            let mut path = folder_path.to_vec();
            path.push(index);
            (path, slot)
        })
        .collect()
}

pub(super) fn hotbar_folder_name<'a>(slots: &'a [HotbarSlot], folder_path: &[usize]) -> &'a str {
    match get_hotbar_slot(slots, folder_path) {
        Some(HotbarSlot::Folder { name, .. }) => name,
        _ => "Hotbar",
    }
}

pub(super) fn get_hotbar_slots<'a>(
    slots: &'a [HotbarSlot],
    folder_path: &[usize],
) -> Option<&'a [HotbarSlot]> {
    match folder_path.split_first() {
        None => Some(slots),
        Some((index, rest)) => match slots.get(*index)? {
            HotbarSlot::Folder { slots, .. } => get_hotbar_slots(slots, rest),
            _ => None,
        },
    }
}

pub(super) fn get_hotbar_slots_mut<'a>(
    slots: &'a mut Vec<HotbarSlot>,
    folder_path: &[usize],
) -> &'a mut Vec<HotbarSlot> {
    match folder_path.split_first() {
        None => slots,
        Some((index, rest)) => match slots
            .get_mut(*index)
            .expect("active hotbar folder path is valid")
        {
            HotbarSlot::Folder { slots, .. } => get_hotbar_slots_mut(slots, rest),
            _ => panic!("active hotbar folder path points to a non-folder"),
        },
    }
}

pub(super) fn get_hotbar_slot<'a>(
    slots: &'a [HotbarSlot],
    path: &[usize],
) -> Option<&'a HotbarSlot> {
    let (index, rest) = path.split_first()?;
    let slot = slots.get(*index)?;
    if rest.is_empty() {
        return Some(slot);
    }
    match slot {
        HotbarSlot::Folder { slots, .. } => get_hotbar_slot(slots, rest),
        _ => None,
    }
}

pub(super) fn find_custom_hotbar_slot_mut(
    slots: &mut [HotbarSlot],
    compiled: Uuid,
) -> Option<&mut HotbarSlot> {
    for slot in slots {
        match slot {
            HotbarSlot::Component {
                compiled: existing, ..
            } if *existing == compiled => return Some(slot),
            HotbarSlot::Folder { slots, .. } => {
                if let Some(found) = find_custom_hotbar_slot_mut(slots, compiled) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn remove_hotbar_slot_at(
    slots: &mut Vec<HotbarSlot>,
    path: &[usize],
) -> Option<HotbarSlot> {
    let (index, parent_path) = path.split_last()?;
    let parent = get_hotbar_slots_mut(slots, parent_path);
    (*index < parent.len()).then(|| parent.remove(*index))
}

pub(super) fn hotbar_slot_drop_target(
    slots: &[HotbarSlot],
    path: &[usize],
) -> Option<HotbarDropTarget> {
    get_hotbar_slot(slots, path).map(|_| HotbarDropTarget::Slot(path.to_vec()))
}

fn followed(path: &[usize], removed: &[usize], inserted: &[usize]) -> Vec<usize> {
    shifted_after_insertion(&shifted_after_removal(path, removed), inserted)
}

pub(super) fn shifted_after_removal(path: &[usize], removed: &[usize]) -> Vec<usize> {
    let mut path = path.to_vec();
    let Some((index, parent)) = removed.split_last() else {
        return path;
    };
    let level = parent.len();
    if path.len() > level && path[..level] == *parent && path[level] > *index {
        path[level] -= 1;
    }
    path
}

pub(super) fn shifted_after_insertion(path: &[usize], inserted: &[usize]) -> Vec<usize> {
    let mut path = path.to_vec();
    let Some((index, parent)) = inserted.split_last() else {
        return path;
    };
    let level = parent.len();
    if path.len() > level && path[..level] == *parent && path[level] >= *index {
        path[level] += 1;
    }
    path
}

pub(super) fn move_hotbar_slot_to_folder(
    slots: &mut Vec<HotbarSlot>,
    source: &[usize],
    folder_path: &[usize],
) -> Option<Vec<usize>> {
    if source == folder_path || folder_path.starts_with(source) {
        return None;
    }
    let slot = remove_hotbar_slot_at(slots, source)?;
    let adjusted_folder = shifted_after_removal(folder_path, source);
    let folder = get_hotbar_slots_mut(slots, &adjusted_folder);
    folder.push(slot);
    let mut placed = adjusted_folder;
    placed.push(folder.len() - 1);
    Some(placed)
}

pub(super) fn move_hotbar_slot(
    slots: &mut Vec<HotbarSlot>,
    source: &[usize],
    target: &[usize],
) -> Option<Vec<usize>> {
    if source == target || target.starts_with(source) {
        return None;
    }
    let slot = remove_hotbar_slot_at(slots, source)?;

    if target.is_empty() {
        slots.push(slot);
        return Some(vec![slots.len() - 1]);
    }

    let adjusted_target = match target.len() > source.len() {
        true => shifted_after_removal(target, source),
        false => target.to_vec(),
    };
    let (index, parent_path) = adjusted_target.split_last()?;
    let parent = get_hotbar_slots_mut(slots, parent_path);
    let index = (*index).min(parent.len());
    parent.insert(index, slot);
    let mut placed = parent_path.to_vec();
    placed.push(index);
    Some(placed)
}

pub(super) fn step_scale(scale: &mut Scale, direction: ScaleDirection) {
    let index = SCALES
        .iter()
        .position(|value| i64::from(*value) == scale.get())
        .expect("selected tool scale is one of the hotbar scales");
    let next = match direction {
        ScaleDirection::Down => index.saturating_sub(1),
        ScaleDirection::Up => (index + 1).min(SCALES.len() - 1),
    };
    *scale = Scale::new(SCALES[next]).expect("hotbar scale is valid");
}

impl LogicGridEditor {
    pub(super) fn hotbar_slot_label(&self, slot: &HotbarSlot) -> String {
        slot.label().to_owned()
    }

    pub(super) fn selected_hotbar_kind(&self, path: &[usize]) -> Option<ComponentKind> {
        get_hotbar_slot(&self.hotbar, path).and_then(|slot| match slot {
            HotbarSlot::Component { kind, .. } => kind.clone(),
            _ => None,
        })
    }

    pub(super) fn hotbar_slot_disabled(&self, slot: &HotbarSlot) -> bool {
        match slot {
            HotbarSlot::Builtin(kind) => self.challenge_tool_exhausted(*kind),
            HotbarSlot::Component { kind, .. } => kind.is_none(),
            _ => false,
        }
    }

    fn challenge_tool_exhausted(&self, kind: ToolKind) -> bool {
        self.challenge.is_some()
            && match kind {
                ToolKind::Input => self.next_missing_challenge_input().is_none(),
                ToolKind::Output => self.next_missing_challenge_output().is_none(),
                _ => false,
            }
    }
}
