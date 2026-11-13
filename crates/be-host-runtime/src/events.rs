//! What an editor instance did that its owners in the host act on, pushed to them as it
//! happens rather than queued per instance for them to poll.

use std::cell::RefCell;

use be_host_base::host;
use be_sdk_api::{ChildId, EditorInstanceId, ViewId};
use beui::Rect;
use uuid::Uuid;

use be_sdk_api::ViewChange;

use crate::{BlockDrag, ChildCommit, EditorAction, Focus};

/// Something an instance did, for whichever owner in the host acts on it.
#[derive(Clone, Debug)]
pub enum InstanceEvent {
    /// It asked to show the details of what it shows.
    Details,
    /// It asked to leave the frame it is shown in.
    Leaving,
    /// It asked the host to move its view.
    ViewChange(ViewChange),
    /// It picked a command of a child's menu; `block` is what the child shows, when it is
    /// placed, and `anchor` is in host coordinates.
    ChildMenuPick {
        child: ChildId,
        block: Option<Uuid>,
        id: String,
        anchor: Option<Rect>,
    },
    /// It started dragging a block out of itself.
    BlockDrag(BlockDrag),
    /// It asked the host to act: open a block, run a command on one.
    Action(EditorAction),
    /// Where its focus is now.
    Focused(Focus),
    /// The artifact blocks it watches now.
    ArtifactWatch(Vec<Uuid>),
    /// How regenerating its artifact went.
    ArtifactOutcome(Result<(), String>),
    /// The block its creation dialog made, or why it could not.
    Created(Result<Uuid, String>),
    /// The person asked to dismiss a child it shows as a dialog.
    ChildDismissed(ChildId),
    /// The person asked to close a child it placed.
    ChildClosed(ChildId),
    /// The person created the block of a creation dialog it placed.
    ChildCommitted(ChildCommit),
    /// Its answer to `render_clip`'s `request`: none when it rendered nothing or is gone.
    Rendered {
        request: u64,
        body: Option<be_sdk_api::clipboard::ClipBody>,
    },
    /// The view `ViewId` was asked to take the keyboard.
    FocusRequested(ViewId),
    /// It gives the keyboard to this child, whose view takes it (`take_gift`) once it can; none
    /// when the gift ended without a child's view taking it, so the keys it held go back.
    KeyboardGift(Option<ChildId>),
    /// The instance closed: nothing more comes from it.
    Closed,
}

/// An event and the instance it is from.
#[derive(Clone, Debug)]
pub struct Heard {
    pub app_id: String,
    pub instance: EditorInstanceId,
    pub event: InstanceEvent,
}

impl Heard {
    pub fn from(&self, app_id: &str, instance: EditorInstanceId) -> bool {
        self.instance == instance && self.app_id == app_id
    }
}

/// Whose events a listener hears: one instance's, or every instance's.
type Key = Option<(String, EditorInstanceId)>;

thread_local! {
    static LISTENERS: RefCell<Vec<(Key, host::Post<Heard>)>> = const { RefCell::new(Vec::new()) };
}

/// Posts the events of the instance `of` names (`(app ID, instance)`), or of every instance
/// for `None`, to `post` from now on, until its mailbox is dropped. Its mailbox hears them
/// when its desk delivers.
pub fn listen(of: Option<(&str, EditorInstanceId)>, post: host::Post<Heard>) {
    let key = of.map(|(app_id, instance)| (app_id.to_owned(), instance));
    LISTENERS.with_borrow_mut(|listeners| listeners.push((key, post)));
}

pub(crate) fn emit(app_id: &str, instance: EditorInstanceId, event: InstanceEvent) {
    // At thread exit (an `Instances` dropped with its app) the listeners may be gone already.
    let _ = LISTENERS.try_with(|listeners| {
        let mut listeners = listeners.borrow_mut();
        listeners.retain(|(_, post)| post.open());
        let hears = |key: &Key| {
            key.as_ref()
                .is_none_or(|(app, of)| *of == instance && app == app_id)
        };
        for (_, post) in listeners.iter().filter(|(key, _)| hears(key)) {
            post.send(Heard {
                app_id: app_id.to_owned(),
                instance,
                event: event.clone(),
            });
        }
    });
}
