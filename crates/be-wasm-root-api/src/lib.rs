//! The root guest's entry (plans/host_as_platform_adapter.md, The root guest): the WIT world
//! `be3:root/root` and the Rust generated from it. The root exports `frames`, whose `frame` the
//! host calls inline on its frame thread and steps until it answers (crates/be-wasm-root-host
//! natively), `input`, whose `deliver` hands it the window's input to route to its surfaces,
//! and `paintings`, what its own surfaces painted, for the host's tests; it imports `host`'s
//! `deadline` and `fonts`; the host also serves it the sandbox (crates/be-wasm-sandbox-api).

pub use be_wasm_ops as ops;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "root",
    deps: ["../be-wasm-ops/wit"],
    with: {
        "uuid": [u8; 16],
        "password": ::be_wasm_password::Password,
    },
    copy: ["uuid"],
    default: [
        "agents-area", "memory-minutes", "contacts-area", "recovery-area", "unlock-area", "core-view", "share-links-area", "invitations-area", "members-area", "organizations-area", "organizations-listed", "sessions-area",
    ],
    secret: ["password"],
});

pub use frames::{FrameDone, FrameRequest, WindowSize};
pub use host::{
    AccountRow, AgentCredentials, AgentRow, AgentsArea, BrowserSignIn, ContactCard,
    ContactMismatch, ContactRow, ContactsArea, CoreCommand, CoreFailure, CoreSecret, CoreView,
    CoreWait, DefaultServer, DiscardReason, DisplayQuestion, DisplayState, EditorApp,
    HostStatistics, HostedBlock, Invitation, InvitationRow, InvitationState, InvitationsArea,
    Leaving, LockState, MemberRole, MemberRow, MembersArea, MemoryMinutes, MemoryPoint, NewAccount,
    Notice, NoticeAction, OpenedEditor, OpenedIn, OrganizationRow, OrganizationsArea, OrganizationsListed,
    PairingApproval, PairingRequest, PowerAction, ProfileRow, Reauthenticating, RecoveryArea,
    Refusal, RelayedFrames, RoleChange, SavingState, SecretCommand, ServerAccount, SessionKind,
    SessionRow, SessionsArea, SetAside, ShareAccess, ShownBlock, ShownChild, ShareLinksArea, SharePreview, ShownNotices,
    SigningIn, StatisticRow, SyncState, UnlockArea, UnsentEdit,
};
