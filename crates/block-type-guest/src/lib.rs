use std::collections::BTreeMap;

use be_block::{BlockContent, LiveEdit, Merge, Undo, history::History};
use be_commit::MergeResult;
use block_type_api::{
    Change, ChildChange, Command, Description, Event, Plain, PlainFile, ReplicaId, Reply, Request,
    TypeInfo, decode_frame, encode_frame,
};
use uuid::Uuid;

pub mod editor;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod entropy;
mod file;
mod record;
mod replica;
pub mod wire;

pub use block_guest_abi as abi;
pub use block_type_api as api;
pub use file::{BlobFile, DocumentFile, FileForm};
pub use record::{NoHistory, Recorder};
pub use replica::{HEARTBEAT_INTERVAL, SEAL_INTERVAL};

use replica::{AnyReplica, Input, Opened, Replica};

type FromFileFn = fn(Option<&[u8]>, &str, Vec<u8>, u64) -> Option<Vec<u8>>;

type MergeFn = fn(Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) -> Result<(Vec<u8>, usize), String>;

type ChildFn = fn(Option<&[u8]>, ChildChange) -> Result<Option<Vec<u8>>, String>;

type ToFileFn = fn(&[u8]) -> Result<Option<PlainFile>, String>;

pub struct Registration {
    info: TypeInfo,
    open: fn(Opened) -> Box<dyn AnyReplica>,
    plain: fn(Opened) -> Box<dyn AnyReplica>,
    blank: fn() -> Vec<u8>,
    merge: MergeFn,
    describe: fn(&[u8], Uuid) -> Result<Description, String>,
    child: ChildFn,
    to_file: Option<ToFileFn>,
    from_file: Option<FromFileFn>,
    claims: Option<fn(&str, &[u8]) -> bool>,
    claims_any_file: bool,
    claims_directories: bool,
}

impl Registration {
    pub fn of<C: LiveEdit + Merge + Clone + Default>() -> Self {
        Self {
            info: TypeInfo {
                content_type: C::CONTENT_TYPE,
                frozen: C::FROZEN,
                schema: be_block::schema_fingerprint::<C>(),
                undo: false,
                plain: Plain::None,
            },
            open: open::<C, NoHistory>,
            plain: open::<C, NoHistory>,
            blank: || C::default().encode(),
            merge: merge::<C>,
            describe: describe_bytes::<C>,
            child: child::<C>,
            to_file: None,
            from_file: None,
            claims: None,
            claims_any_file: false,
            claims_directories: false,
        }
    }

    pub fn undo<C: Undo + Merge + Clone + Default>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.undo = true;
        self.open = open::<C, History<C, u64>>;
        self
    }

    pub fn without_undo(mut self) -> Self {
        self.info.undo = false;
        self.open = self.plain;
        self
    }

    pub fn file<C: FileForm + LiveEdit>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.plain = Plain::File;
        self.to_file = Some(file::to_file::<C>);
        self.from_file = Some(file::from_file::<C>);
        self.claims = Some(C::claims);
        self.claims_any_file = C::ANY_FILE;
        self
    }

    pub fn directory<C: LiveEdit>(mut self) -> Self {
        self = self.root::<C>();
        self.claims_directories = true;
        self
    }

    pub fn root<C: LiveEdit>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.plain = Plain::Directory;
        self
    }

    pub fn info(&self) -> &TypeInfo {
        &self.info
    }
}

fn open<C, H>(opened: Opened) -> Box<dyn AnyReplica>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C> + 'static,
{
    Box::new(Replica::<C, H>::new(opened))
}

fn merge<C: Merge + Default>(
    base: Option<&[u8]>,
    ours: Option<&[u8]>,
    theirs: Option<&[u8]>,
) -> Result<(Vec<u8>, usize), String> {
    let decode = |bytes: Option<&[u8]>| match bytes {
        Some(bytes) => C::decode(bytes).map_err(|error| error.to_string()),
        None => Ok(C::default()),
    };
    Ok(
        match C::merge3(&decode(base)?, &decode(ours)?, &decode(theirs)?) {
            MergeResult::Clean(value) => (value.encode(), 0),
            MergeResult::Conflicted { value, conflicts } => (value.encode(), conflicts.max(1)),
        },
    )
}

fn child<C: LiveEdit + Default>(
    bytes: Option<&[u8]>,
    change: ChildChange,
) -> Result<Option<Vec<u8>>, String> {
    let mut content = match bytes {
        Some(bytes) => C::decode(bytes).map_err(|error| error.to_string())?,
        None => C::default(),
    };
    let Some(operations) = content.child_operations(be_child(change)) else {
        return Ok(None);
    };
    for operation in &operations {
        content.apply(operation);
    }
    Ok(Some(content.encode()))
}

pub(crate) fn be_child(change: ChildChange) -> be_block::ChildChange {
    match change {
        ChildChange::Add(block) => be_block::ChildChange::Add(block),
        ChildChange::Delete(block) => be_block::ChildChange::Delete(block),
        ChildChange::Replace { old, new } => be_block::ChildChange::Replace { old, new },
    }
}

pub fn describe<C: BlockContent>(content: &C, organization: Uuid) -> Description {
    Description {
        name: content.name(),
        derived: be_block::derived_metadata(content),
        references: content.references_in(organization),
    }
}

fn describe_bytes<C: BlockContent>(
    bytes: &[u8],
    organization: Uuid,
) -> Result<Description, String> {
    let content = C::decode(bytes).map_err(|error| error.to_string())?;
    Ok(describe(&content, organization))
}

pub struct Module {
    types: Vec<Registration>,
    replicas: BTreeMap<ReplicaId, Box<dyn AnyReplica>>,
    now: u64,
    wake: Option<u64>,
}

impl Module {
    pub fn new(types: Vec<Registration>) -> Self {
        Self {
            types,
            replicas: BTreeMap::new(),
            now: 0,
            wake: None,
        }
    }

    pub fn hello(&self) -> Command {
        Command::Hello {
            types: self.types.iter().map(|kind| kind.info.clone()).collect(),
        }
    }

    pub fn register(&mut self, registration: Registration) {
        let content_type = registration.info.content_type;
        match self
            .types
            .iter_mut()
            .find(|kind| kind.info.content_type == content_type)
        {
            Some(held) => *held = registration,
            None => self.types.push(registration),
        }
    }

    pub fn knows(&self, content_type: Uuid) -> bool {
        self.kind(content_type).is_some()
    }

    pub fn is_open(&self, replica: ReplicaId) -> bool {
        self.replicas.contains_key(&replica)
    }

    pub fn handle(&mut self, event: Event) -> Vec<Command> {
        let mut out = Vec::new();
        match event {
            Event::Entropy { seed } => {
                #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
                entropy::seed(seed);
                #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
                let _ = seed;
            }
            Event::Tick { now } => {
                self.now = self.now.max(now);
                for replica in self.replicas.values_mut() {
                    replica.tick(self.now, &mut out);
                }
            }
            Event::Open {
                replica,
                block: _,
                content_type,
                organization,
                client,
                tag,
                head,
                session,
            } => match self
                .types
                .iter()
                .find(|kind| kind.info.content_type == content_type)
            {
                Some(kind) => {
                    let mut opened = (kind.open)(Opened {
                        replica,
                        content_type,
                        organization,
                        client,
                        tag,
                        head,
                        session,
                        from_file: kind.from_file,
                        now: self.now,
                    });
                    opened.start(&mut out);
                    self.replicas.insert(replica, opened);
                }
                None => {
                    out.push(Command::Error {
                        replica,
                        message: format!("this module has no block type {content_type}"),
                    });
                    out.push(Command::Release { replica });
                }
            },
            Event::Message {
                replica,
                from,
                bytes,
            } => self.input(replica, Input::Message { from, bytes }, &mut out),
            Event::Attach {
                replica,
                editor,
                editable,
            } => self.input(replica, Input::Attach { editor, editable }, &mut out),
            Event::Detach { replica, editor } => {
                self.input(replica, Input::Detach { editor }, &mut out);
            }
            Event::Head { replica, event } => self.input(replica, Input::Head(event), &mut out),
            Event::Lease {
                replica,
                session,
                resynchronize,
            } => self.input(
                replica,
                Input::Lease {
                    session,
                    resynchronize,
                },
                &mut out,
            ),
            Event::Read {
                replica,
                token,
                result,
            } => self.input(replica, Input::Read { token, result }, &mut out),
            Event::Resumed {
                replica,
                token,
                result,
            } => self.input(replica, Input::Resumed { token, result }, &mut out),
            Event::Close { replica } => self.input(replica, Input::Close, &mut out),
            Event::Claim { replica } => self.input(replica, Input::Claim, &mut out),
            Event::Request { id, request } => self.request(id, request, &mut out),
        }
        self.replicas.retain(|_, replica| !replica.released());
        let wake = self
            .replicas
            .values()
            .filter_map(|replica| replica.wake_at())
            .min();
        if wake.is_some() && wake != self.wake {
            out.push(Command::WakeAt {
                at: wake.unwrap_or_default(),
            });
        }
        self.wake = wake;
        out
    }

    fn input(&mut self, replica: ReplicaId, input: Input, out: &mut Vec<Command>) {
        match self.replicas.get_mut(&replica) {
            Some(open) => open.input(self.now, input, out),
            None => {
                if let Input::Change { id, .. } | Input::Content { id } | Input::Seal { id, .. } =
                    input
                {
                    out.push(Command::Reply {
                        id,
                        reply: Reply::Unknown,
                    });
                }
            }
        }
    }

    fn kind(&self, content_type: Uuid) -> Option<&Registration> {
        self.types
            .iter()
            .find(|kind| kind.info.content_type == content_type)
    }

    fn request(&mut self, id: u64, request: Request, out: &mut Vec<Command>) {
        let reply = match request {
            Request::Change { replica, change } => {
                return self.input(replica, Input::Change { id, change }, out);
            }
            Request::Content { replica } => return self.input(replica, Input::Content { id }, out),
            Request::Seal { replica, force } => {
                return self.input(replica, Input::Seal { id, force }, out);
            }
            Request::Claims {
                name,
                first,
                directory,
                fallback,
            } => Reply::Claimed(
                self.types
                    .iter()
                    .find(|kind| match directory {
                        true => kind.claims_directories && !fallback,
                        false => {
                            kind.claims_any_file == fallback
                                && kind.claims.is_some_and(|claims| claims(&name, &first))
                        }
                    })
                    .map(|kind| kind.info.content_type),
            ),
            Request::Apply {
                content_type,
                bytes,
                client,
                change,
            } => match self.kind(content_type) {
                Some(kind) => apply(kind, bytes.as_deref(), client, change),
                None => Reply::Unknown,
            },
            Request::Blank { content_type } => match self.kind(content_type) {
                Some(kind) => Reply::Blank((kind.blank)()),
                None => Reply::Unknown,
            },
            Request::Merge3 {
                content_type,
                base,
                ours,
                theirs,
            } => match self.kind(content_type) {
                Some(kind) => {
                    match (kind.merge)(base.as_deref(), ours.as_deref(), theirs.as_deref()) {
                        Ok((bytes, conflicts)) => Reply::Merged { bytes, conflicts },
                        Err(error) => Reply::Malformed(error),
                    }
                }
                None => Reply::Unknown,
            },
            Request::Describe {
                content_type,
                bytes,
                organization,
            } => match self.kind(content_type) {
                Some(kind) => match (kind.describe)(&bytes, organization) {
                    Ok(description) => Reply::Described(description),
                    Err(error) => Reply::Malformed(error),
                },
                None => Reply::Unknown,
            },
            Request::ToFile {
                content_type,
                bytes,
            } => match self.kind(content_type) {
                Some(kind) => match kind.to_file.map(|to_file| to_file(&bytes)) {
                    None => Reply::File(None),
                    Some(Ok(file)) => Reply::File(file),
                    Some(Err(error)) => Reply::Malformed(error),
                },
                None => Reply::Unknown,
            },
        };
        out.push(Command::Reply { id, reply });
    }
}

fn apply(kind: &Registration, bytes: Option<&[u8]>, client: u64, change: Change) -> Reply {
    let applied = match change {
        Change::Child(change) => (kind.child)(bytes, change),
        Change::FromFile { name, bytes: file } => match kind.from_file {
            Some(from_file) => Ok(from_file(bytes, &name, file, client)),
            None => Ok(None),
        },
        Change::Replace(_) | Change::Undo | Change::Redo => return Reply::Unknown,
    };
    match applied {
        Ok(Some(bytes)) => Reply::Content(bytes),
        Ok(None) => Reply::Changed(false),
        Err(error) => Reply::Malformed(error),
    }
}

impl Module {
    pub fn hello_frame(&self) -> Vec<u8> {
        encode_frame(&self.hello())
    }

    pub fn step(&mut self, frames: Vec<Vec<u8>>, outbound: &mut Vec<Vec<u8>>) {
        for frame in frames {
            match decode_frame::<Event>(&frame) {
                Ok(event) => outbound.extend(self.handle(event).iter().map(encode_frame)),
                Err(error) => outbound.push(encode_frame(&Command::Error {
                    replica: 0,
                    message: error.to_string(),
                })),
            }
        }
    }
}

#[macro_export]
macro_rules! be_block_types {
    ($($kind:ty $({ $($part:ident),* $(,)? })?),* $(,)?) => {
        pub fn registrations() -> ::std::vec::Vec<$crate::Registration> {
            ::std::vec![
                $($crate::Registration::of::<$kind>()$($(.$part::<$kind>())*)?),*
            ]
        }

        pub fn block_types() -> $crate::Module {
            $crate::Module::new(registrations())
        }

        pub struct BlockTypes($crate::Module);

        impl $crate::abi::Guest for BlockTypes {
            fn start(
                _: $crate::abi::Waker,
                outbound: &mut ::std::vec::Vec<::std::vec::Vec<u8>>,
            ) -> ::std::result::Result<Self, ::std::string::String> {
                let module = block_types();
                outbound.push(module.hello_frame());
                ::std::result::Result::Ok(Self(module))
            }

            fn step(
                &mut self,
                inbound: ::std::vec::Vec<::std::vec::Vec<u8>>,
                _: bool,
                outbound: &mut ::std::vec::Vec<::std::vec::Vec<u8>>,
            ) -> ::std::result::Result<$crate::abi::Flow, ::std::string::String> {
                self.0.step(inbound, outbound);
                ::std::result::Result::Ok($crate::abi::Flow::Running)
            }
        }
    };
}

#[cfg(test)]
mod tests;
