use std::cell::Cell;

use beui::accesskit::Action;
use beui::native::accessibility::Fragment;
use beui::{ActionGroup, ActionInfo};
use block_plugin_api::{ScreenId, ViewPlacement};

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
    actions: ActionGroup,
}

pub fn describe(context: &beui::native::Context) {
    let on = context.accessibility_active()
        && (context.automated() || context.accessibility_requested());
    let was = DESCRIBING.with(|describing| describing.replace(on));
    if !on {
        return;
    }
    if !was {
        context.request_repaint();
    }
    let mut regions = Vec::new();
    runtime::each(|runtime| {
        let plugin = runtime.id().to_owned();
        let name = runtime.name().to_owned();
        let placements: Vec<_> = runtime
            .placements
            .iter()
            .filter(|((instance, view), _)| {
                runtime
                    .instances
                    .view_placement(*instance, *view)
                    .is_some_and(ViewPlacement::reports)
            })
            .map(|((instance, region), placed)| {
                (
                    *instance,
                    *region,
                    placed.rect,
                    placed.clip,
                    placed.zoom,
                    false,
                )
            })
            .collect();
        let anchored: Vec<_> = placements
            .iter()
            .flat_map(|(instance, region, ..)| {
                runtime
                    .instances
                    .anchored_views(*instance, *region)
                    .into_iter()
                    .map(|(view, rect, bounds)| (*instance, view, rect, bounds, 1.0, true))
            })
            .collect();
        for (instance, region, rect, clip, zoom, anchored) in placements.into_iter().chain(anchored)
        {
            let title =
                (!anchored && !super::docks::is_bar(instance, region.0)).then_some(name.as_str());
            let Some(described) = runtime.instances.described(instance, region) else {
                continue;
            };
            let document = described.tree.document();
            let screen = described.screen;
            let requests: Vec<_> = context
                .take_accessibility_actions(document)
                .into_iter()
                .filter_map(|mut request| {
                    request.target_node = described.tree.plugin_node(request.target_node)?;
                    Some(request)
                })
                .collect();
            let fragment = described.tree.fragment(
                title,
                rect.intersect(clip),
                rect.min.to_vec2(),
                zoom,
                described.focused,
                !context.accessibility_known(document),
            );
            let actions = ActionGroup {
                name: name.clone(),
                window: None,
                actions: described
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
            };
            regions.push(Region {
                plugin: plugin.clone(),
                screen,
                document,
                fragment,
                actions,
            });
            for request in requests {
                if matches!(
                    request.action,
                    Action::Focus | Action::Click | Action::SetValue | Action::ReplaceSelectedText
                ) {
                    runtime.instances.request_focus(screen);
                    runtime::mark(&plugin);
                }
                runtime.node_action(screen, request);
            }
        }
    });
    regions.sort_by_key(|region| region.document);
    for (_, requested) in context.action_requests() {
        let (pane, id) = match requested.split_once(beui_host::native::drive::PANE) {
            Some((pane, id)) => (Some(pane), id),
            None => (None, requested.as_str()),
        };
        let offers = |region: &&Region, live: bool| {
            pane.is_none_or(|pane| region.actions.name == pane)
                && region
                    .actions
                    .actions
                    .iter()
                    .any(|action| action.id == id && (action.live || !live))
        };
        let owner = regions
            .iter()
            .rev()
            .find(|region| offers(region, true))
            .or_else(|| regions.iter().rev().find(|region| offers(region, false)));
        if let Some(owner) = owner {
            let screen = owner.screen;
            let id = id.to_owned();
            runtime::with(&owner.plugin, |runtime| runtime.run_action(screen, id));
            context.action_ran(&requested);
        }
    }
    for region in regions {
        context.publish_accessibility(region.document, region.fragment);
        context.publish_actions(region.actions);
    }
}
