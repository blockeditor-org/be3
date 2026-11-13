//! How version control reaches the open replicas and the type modules, which the worker's loop
//! owns: directly while a status is computed in the loop, and through the loop (`LoopCalls`)
//! while a version command runs in the lane beside it (guides/blocks/host.md, Commands that
//! need the server). Each call through the loop is short, a module's answer or a replica's
//! head; what waits on the server stays in the lane.

use std::{future::Future, pin::Pin, sync::Arc};

use be_block_client::{ClientError, Peer};
use be_block_commit::CommitId;
use be_block_store::Manifest;
use be_block_type_host::{Kind, Merged};
use be_sdk_type_api::{Change, ClientId, Description};
use be_wasm_threads::Mutex;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

use crate::{
    replicas::Replicas,
    worker::{Shared, Store},
};

type Ctx<'a> = (&'a Arc<Peer<Store>>, &'a Arc<Mutex<Shared>>);

/// Work for the loop to do with its replicas, handed over by the lane.
pub(crate) type LoopCall = Box<
    dyn for<'a> FnOnce(
        &'a mut Replicas,
        &'a Arc<Peer<Store>>,
        &'a Arc<Mutex<Shared>>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>,
>;

/// Hands calls to the worker's loop, which runs each when it next wakes.
#[derive(Clone)]
pub(crate) struct LoopCalls(pub(crate) UnboundedSender<LoopCall>);

impl LoopCalls {
    async fn call<T: 'static>(
        &self,
        work: impl for<'a> FnOnce(&'a mut Replicas, Ctx<'a>) -> Pin<Box<dyn Future<Output = T> + 'a>>
        + 'static,
    ) -> Result<T, String> {
        let (answer, answered) = oneshot::channel();
        let call: LoopCall = Box::new(move |replicas, peer, shared| {
            Box::pin(async move {
                let _ = answer.send(work(replicas, (peer, shared)).await);
            })
        });
        self.0
            .send(call)
            .map_err(|_| "The block store stopped.".to_owned())?;
        be_host_base::host::wake();
        answered
            .await
            .map_err(|_| "The block store stopped.".to_owned())
    }
}

pub(crate) enum Reach<'a> {
    /// In the loop, which owns the replicas.
    Here(&'a mut Replicas),
    /// In the lane: through the loop.
    Loop(&'a LoopCalls),
}

impl Reach<'_> {
    pub(crate) async fn kind(&mut self, content_type: Uuid) -> Option<Kind> {
        match self {
            Reach::Here(replicas) => replicas.kind(content_type).await,
            Reach::Loop(calls) => calls
                .call(move |replicas, _| Box::pin(replicas.kind(content_type)))
                .await
                .ok()
                .flatten(),
        }
    }

    pub(crate) async fn apply(
        &mut self,
        ctx: Ctx<'_>,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> Result<Option<Vec<u8>>, String> {
        match self {
            Reach::Here(replicas) => {
                replicas
                    .apply(ctx, content_type, bytes, client, change)
                    .await
            }
            Reach::Loop(calls) => calls
                .call(move |replicas, ctx| {
                    Box::pin(replicas.apply(ctx, content_type, bytes, client, change))
                })
                .await
                .and_then(|answer| answer),
        }
    }

    pub(crate) async fn describe(
        &mut self,
        ctx: Ctx<'_>,
        content_type: Uuid,
        bytes: &[u8],
    ) -> Option<Description> {
        match self {
            Reach::Here(replicas) => replicas.describe(ctx, content_type, bytes).await,
            Reach::Loop(calls) => {
                let bytes = bytes.to_vec();
                calls
                    .call(move |replicas, ctx| {
                        Box::pin(async move { replicas.describe(ctx, content_type, &bytes).await })
                    })
                    .await
                    .ok()
                    .flatten()
            }
        }
    }

    /// Merges three sides of a block's content. Through the loop, the sides are read from the
    /// server here first, so the loop finds them on the device.
    pub(crate) async fn merge(
        &mut self,
        ctx: Ctx<'_>,
        content_type: Uuid,
        sides: [Option<&Manifest>; 3],
    ) -> Result<Option<Merged>, ClientError> {
        match self {
            Reach::Here(replicas) => replicas.merge(ctx, content_type, sides).await,
            Reach::Loop(calls) => {
                for side in sides.into_iter().flatten() {
                    ctx.0.read_manifest(side).await?;
                }
                let sides = sides.map(|side| side.cloned());
                calls
                    .call(move |replicas, ctx| {
                        Box::pin(async move {
                            replicas
                                .merge(ctx, content_type, sides.each_ref().map(Option::as_ref))
                                .await
                        })
                    })
                    .await
                    .map_err(|error| ClientError::Disconnected(error))?
            }
        }
    }

    pub(crate) async fn open(
        &mut self,
        ctx: Ctx<'_>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
    ) {
        match self {
            Reach::Here(replicas) => replicas.open(ctx, block, content_type, reader).await,
            Reach::Loop(calls) => {
                let _ = calls
                    .call(move |replicas, ctx| {
                        Box::pin(replicas.open(ctx, block, content_type, reader))
                    })
                    .await;
            }
        }
    }

    /// Moves an open replica to a head this device published.
    pub(crate) async fn moved_here(&mut self, block: Uuid, head: CommitId) {
        match self {
            Reach::Here(replicas) => replicas.moved_here(block, head),
            Reach::Loop(calls) => {
                let _ = calls
                    .call(move |replicas, _| {
                        replicas.moved_here(block, head);
                        Box::pin(std::future::ready(()))
                    })
                    .await;
            }
        }
    }
}
