use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Rect};

use super::{DesktopNotifications, notification};

#[component]
pub(crate) fn DesktopToasts(
    notifications: DesktopNotifications,
    area: Memo<Rect>,
    corner: Memo<Edge>,
) -> NodeId {
    let anchor = create_memo(move || OverlayAnchor::Rect(area.get()));
    let toasts = notifications.toasts();
    view! {
        <Toasts
            anchor={anchor}
            edge={corner}
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
