use std::cell::Cell;
use std::collections::HashSet;

use beui::accessibility::{Described, embedded, role_named};
use beui::{Rect, pos2, vec2};
use block_plugin_api::{ChildRect, EditorRegion, Toggled};

use super::runtime;

thread_local! {
    static DESCRIBING: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn describing() -> bool {
    DESCRIBING.with(Cell::get)
}

pub(crate) fn describe(context: &beui::Context) {
    let on = context.automated();
    DESCRIBING.with(|describing| describing.set(on));
    if !on {
        return;
    }
    let mut regions = Vec::new();
    runtime::each(|runtime| {
        for ((instance, region), placed) in &runtime.placements {
            if *region != EditorRegion::Frame {
                continue;
            }
            let Some(description) = runtime
                .instances
                .frame_report(*instance, *region)
                .and_then(|report| report.description.as_ref())
            else {
                continue;
            };
            let origin = placed.rect.min;
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
            let area = placed.rect.intersect(placed.clip);
            let test_ids: Vec<(String, Rect)> = description
                .test_ids
                .iter()
                .map(|test_id| (test_id.id.clone(), place(&test_id.rect)))
                .collect();
            regions.push((
                placed.document,
                embedded(placed.document, runtime.name(), area, &nodes),
                test_ids,
            ));
        }
    });
    regions.sort_by_key(|(document, _, _)| *document);
    let mut published = HashSet::new();
    for (document, fragment, test_ids) in regions {
        context.publish_accessibility(document, fragment);
        for (id, rect) in test_ids {
            match published.insert(id.clone()) {
                true => context.publish_test_id(&id, rect),
                false => context.publish_ambiguous_test_id(&id),
            }
        }
    }
}
