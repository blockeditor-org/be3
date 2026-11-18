use super::*;
use be_sdk_api::{
    blocks::BlockHistory,
    embed::blocks as wire,
    ops::{Connection, Side, decode},
};

mod frames_go_to_the_latest_replica_of_a_block_and_the_earlier_resumes;
mod a_watch_is_not_answered_again_what_it_was_answered;
mod the_store_refuses_a_call_before_it_is_dispatched;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);

/// A store of one instance that records what reaches it.
#[derive(Default)]
struct Recorded {
    blocks: EditorBlocks,
    heard: Vec<String>,
    refuse: bool,
}

impl BlockStore for Recorded {
    fn held(&mut self, instance: EditorInstanceId) -> Option<&mut EditorBlocks> {
        (instance == INSTANCE).then_some(&mut self.blocks)
    }

    fn holder(&self, op: OpId) -> Option<EditorInstanceId> {
        self.blocks.holds(op).then_some(INSTANCE)
    }

    fn admit(&mut self, _: EditorInstanceId, _: &mut Call) -> Result<(), OpError> {
        match self.refuse {
            true => Err(OpError::Denied),
            false => Ok(()),
        }
    }

    fn open_block(&mut self, _: EditorInstanceId, _: OpenRequest) -> Result<bool, OpError> {
        self.heard.push("open".into());
        Ok(true)
    }

    fn act(&mut self, _: EditorInstanceId, _: Uuid, _: BlockCommand) -> Result<bool, OpError> {
        self.heard.push("act".into());
        Ok(true)
    }

    fn create_block(&mut self, _: EditorInstanceId, _: Created) -> Result<bool, OpError> {
        self.heard.push("create".into());
        Ok(true)
    }

    fn write_content(&mut self, _: EditorInstanceId, _: Written) -> Result<bool, OpError> {
        self.heard.push("write".into());
        Ok(true)
    }

    fn replica_opened(&mut self, _: EditorInstanceId, op: OpId, block: Uuid, _: Uuid) -> bool {
        self.heard.push(format!("opened {op} {block}"));
        true
    }

    fn replica_sent(&mut self, _: EditorInstanceId, block: Uuid, _: Vec<u8>) -> bool {
        self.heard.push(format!("sent {block}"));
        true
    }

    fn replica_resumed(&mut self, _: EditorInstanceId, op: OpId, block: Uuid, content_type: Uuid) {
        self.heard.push(format!("resumed {op} {block} {content_type}"));
    }

    fn replica_ended(&mut self, _: EditorInstanceId, block: Uuid) {
        self.heard.push(format!("ended {block}"));
    }

    fn watch_opened(&mut self, _: EditorInstanceId, op: OpId, _: Subject) -> bool {
        self.heard.push(format!("watching {op}"));
        true
    }
}

thread_local! {
    static HOST: Connection = Connection::new(Side::Host);
}

fn reply<T, S>(op: OpId) -> be_sdk_api::ops::Reply<T, S> {
    HOST.with(|host| be_sdk_api::ops::Reply::new(host, op))
}

fn open_replica(store: &mut Recorded, op: OpId, block: Uuid) {
    open_replica_of(store, op, block, Uuid::nil());
}

fn open_replica_of(store: &mut Recorded, op: OpId, block: Uuid, content_type: Uuid) {
    call(
        store,
        INSTANCE,
        Call::ReplicaNew {
            resource: wire::Replica(op),
            block: block.into_bytes(),
            content_type: content_type.into_bytes(),
        },
    );
}

fn send(store: &mut Recorded, replica: OpId, op: OpId) -> Dispatched {
    call(
        store,
        INSTANCE,
        Call::ReplicaSend {
            this: wire::Replica(replica),
            frame: Vec::new(),
            reply: reply(op),
        },
    )
}

fn history(can_undo: bool) -> BlockValue {
    BlockValue::History(BlockHistory {
        can_undo,
        ..BlockHistory::default()
    })
}
