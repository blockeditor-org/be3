use std::cell::Cell;
use std::collections::HashSet;

use beui::accessibility::{Described, Fragment, embedded, role_named};
use beui::{ActionGroup, ActionInfo, Rect, pos2, vec2};
use block_plugin_api::{ChildRect, EditorRegion, ScreenId, Toggled};

use super::runtime;

thread_local! {
    static DESCRIBING: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn describing() -> bool {
    DESCRIBING.with(Cell::get)
}

struct Region {
    plugin: String,
    screen: ScreenId,
    document: u32,
    fragment: Fragment,
    test_ids: Vec<(String, Rect)>,
    actions: ActionGroup,
}

pub(crate) fn describe(context: &beui::Context) {
    let on = context.automated();
    let was = DESCRIBING.with(|describing| describing.replace(on));
    if !on {
        return;
    }
    if !was {
        context.request_repaint();
    }
    let mut regions = Vec::new();
    runtime::each(|runtime| {
        for ((instance, region), placed) in &runtime.placements {
            if *region != EditorRegion::Frame {
                continue;
            }
            let Some(report) = runtime.instances.frame_report(*instance, *region) else {
                continue;
            };
            let Some(description) = &report.description else {
                continue;
            };
            let origin = placed.rect.min;
            let area = placed.rect.intersect(placed.clip);
            let place = |rect: &ChildRect| {
                Rect::from_min_size(
                    pos2(origin.x + rect.x, origin.y + rect.y),
                    vec2(rect.width, rect.height),
                )
            };
            let nodes: Vec<Described> = description
                .nodes
                .iter()
                .map(|node| Described {
                    depth: usize::from(node.depth),
                    role: role_named(&node.role),
                    label: node.label.clone(),
                    value: node.value.clone(),
                    toggled: node.toggled.map(|toggled| match toggled {
                        Toggled::Off => beui::accesskit::Toggled::False,
                        Toggled::On => beui::accesskit::Toggled::True,
                        Toggled::Mixed => beui::accesskit::Toggled::Mixed,
                    }),
                    disabled: node.disabled,
                    focused: node.focused,
                    rect: node.rect.as_ref().map(place),
                })
                .collect();
            regions.push(Region {
                plugin: runtime.id().to_owned(),
                screen: report.screen,
                document: placed.document,
                fragment: embedded(placed.document, runtime.name(), area, &nodes),
                test_ids: description
                    .test_ids
                    .iter()
                    .map(|test_id| (test_id.id.clone(), place(&test_id.rect).intersect(area)))
                    .filter(|(_, rect)| rect.is_positive())
                    .collect(),
                actions: ActionGroup {
                    name: runtime.name().to_owned(),
                    actions: description
                        .actions
                        .iter()
                        .map(|action| ActionInfo {
                            id: action.id.clone(),
                            label: action.label.clone(),
                            shortcut: action.shortcut.clone(),
                            enabled: action.enabled,
                            checked: action.checked,
                            live: action.live,
                        })
                        .collect(),
                },
            });
        }
    });
    regions.sort_by_key(|region| region.document);
    for requested in context.action_requests() {
        let owner = regions
            .iter()
            .rev()
            .find(|region| {
                region
                    .actions
                    .actions
                    .iter()
                    .any(|action| action.id == requested && action.live)
            })
            .or_else(|| {
                regions.iter().rev().find(|region| {
                    region
                        .actions
                        .actions
                        .iter()
                        .any(|action| action.id == requested)
                })
            });
        if let Some(owner) = owner {
            let screen = owner.screen;
            let id = requested.clone();
            runtime::with(&owner.plugin, |runtime| runtime.run_action(screen, id));
            context.action_ran(&requested);
        }
    }
    let mut published = HashSet::new();
    for region in regions {
        context.publish_accessibility(region.document, region.fragment);
        context.publish_actions(region.actions);
        for (id, rect) in region.test_ids {
            match published.insert(id.clone()) {
                true => context.publish_test_id(&id, rect),
                false => context.publish_ambiguous_test_id(&id),
            }
        }
    }
}
