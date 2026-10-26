use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Rect, pos2};

use super::{DesktopNotifications, notification};

pub(crate) fn above_bar(screen: Rect, bar: Rect) -> Rect {
    let covers = bar.is_positive()
        && bar.left() < screen.right()
        && bar.right() > screen.left()
        && bar.top() < screen.bottom()
        && bar.bottom() > screen.top();
    match covers {
        true => Rect::from_min_max(
            screen.min,
            pos2(screen.right(), bar.top().max(screen.top())),
        ),
        false => screen,
    }
}

#[component]
pub(crate) fn DesktopToasts(
    notifications: DesktopNotifications,
    bar: NodeRef,
    area: Memo<Rect>,
) -> NodeId {
    let bar = bar.try_get().map(node_rect);
    let anchor = create_memo(move || {
        let bar = bar.as_ref().map_or(Rect::ZERO, |bar| bar.get());
        OverlayAnchor::Rect(above_bar(area.get(), bar))
    });
    let toasts = notifications.toasts();
    view! {
        <Toasts
            anchor={anchor}
            toasts={toasts}
            on_dismiss={move |toast: u64| {
                if let Some(id) = notification(toast) {
                    notifications.dismiss(&[id]);
                }
            }}
            on_action={move |(toast, key): (u64, String)| {
                if let Some(id) = notification(toast) {
                    notifications.invoke(id, &key);
                }
            }}
            on_activate={move |toast: u64| {
                if let Some(id) = notification(toast) {
                    notifications.activate(id);
                }
            }}
        />
    }
}
