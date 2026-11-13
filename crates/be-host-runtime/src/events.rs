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
    /// It picked a command of a child's menu; `anchor` is in host coordinates.
    ChildMenuPick {
        child: ChildId,
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
    Rendered { request: u64, data: Option<Vec<u8>> },
    /// The view `ViewId` was asked to take the keyboard.
    FocusRequested(ViewId),
    /// It gives the keyboard to this child, whose view takes it (`take_gift`) once it can; none
    /// when the gift ended without a child's view taking it, so the keys it held go back.
    KeyboardGift(Option<ChildId>),
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

thread_local! {
    static LISTENERS: RefCell<Vec<host::Post<Heard>>> = const { RefCell::new(Vec::new()) };
}

/// Posts every event of every instance to `post` from now on, until its mailbox is dropped.
/// Its mailbox hears them when its desk delivers.
pub fn listen(post: host::Post<Heard>) {
    LISTENERS.with_borrow_mut(|listeners| listeners.push(post));
}

pub(crate) fn emit(app_id: &str, instance: EditorInstanceId, event: InstanceEvent) {
    LISTENERS.with_borrow_mut(|listeners| {
        listeners.retain(host::Post::open);
        for listener in listeners.iter() {
            listener.send(Heard {
                app_id: app_id.to_owned(),
                instance,
                event: event.clone(),
            });
        }
    });
}
