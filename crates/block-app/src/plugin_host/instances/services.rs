use std::{collections::HashMap, marker::PhantomData};

use block_plugin_api::{
    AccessListing, BlockIdRole, EditorInstanceId, Failure, FileSave, ImportedBlock, SavedFile,
    embed::host_services::Call,
    ops::{OpError, OpId, Outcome, Reply},
};
use serde::Serialize;
use uuid::Uuid;

use super::{Instances, allowed};
use crate::{
    be::revisions,
    editors::plugin::discovery,
    host,
    platform::http::{self, AbortFlag, Aborted},
    plugin_host::wire::Service,
};

type Visit<'a> = dyn FnMut(&mut [u8; 16]) + 'a;

struct Delivered {
    epoch: u64,
    instance: EditorInstanceId,
    op: OpId,
    answer: Answer,
}

enum Answer {
    Done(Outcome),
    Previewed(Result<Uuid, Failure>),
    Imported(Result<Option<ImportedBlock>, Failure>),
    Listed(Result<AccessListing, Failure>),
}

struct Work {
    instance: EditorInstanceId,
    aborted: Aborted,
}

pub(super) struct Services {
    sender: host::WakingSender<Delivered>,
    receiver: std::sync::mpsc::Receiver<Delivered>,
    serving: HashMap<OpId, Work>,
    answers: Vec<(OpId, Outcome)>,
}

impl Default for Services {
    fn default() -> Self {
        let (sender, receiver) = host::waking_channel();
        Self {
            sender,
            receiver,
            serving: HashMap::new(),
            answers: Vec::new(),
        }
    }
}

struct Answering<T> {
    sender: host::WakingSender<Delivered>,
    epoch: u64,
    instance: EditorInstanceId,
    op: OpId,
    aborted: AbortFlag,
    answer: PhantomData<fn(T)>,
}

impl<T> Answering<T> {
    fn withdrawn(&self) -> bool {
        self.aborted.aborted()
    }

    fn deliver(self, answer: Answer) {
        let _ = self.sender.send(Delivered {
            epoch: self.epoch,
            instance: self.instance,
            op: self.op,
            answer,
        });
    }
}

impl<T: Serialize> Answering<T> {
    fn send(self, answer: T) {
        if !self.withdrawn() {
            let outcome = Outcome::returned(&answer);
            self.deliver(Answer::Done(outcome));
        }
    }
}

impl<T: Serialize> Answering<Result<T, Failure>> {
    fn send_or_fail(self, answer: Result<T, String>) {
        self.send(answer.map_err(Failure::Failed));
    }
}

impl Answering<Result<[u8; 16], Failure>> {
    fn previewed(self, previewed: Result<Uuid, String>) {
        self.deliver(Answer::Previewed(previewed.map_err(Failure::Failed)));
    }
}

impl Answering<Result<Option<ImportedBlock>, Failure>> {
    fn imported(self, imported: Result<Option<ImportedBlock>, String>) {
        if !self.withdrawn() {
            self.deliver(Answer::Imported(imported.map_err(Failure::Failed)));
        }
    }
}

impl Answering<Result<AccessListing, Failure>> {
    fn listed(self, listed: Result<AccessListing, String>) {
        if !self.withdrawn() {
            self.deliver(Answer::Listed(listed.map_err(Failure::Failed)));
        }
    }
}

impl Instances {
    fn work(&mut self, instance: EditorInstanceId, op: OpId) -> Aborted {
        let aborted = Aborted::default();
        let work = Work {
            instance,
            aborted: aborted.clone(),
        };
        self.services.serving.insert(op, work);
        aborted
    }

    fn answering<T: Serialize>(
        &mut self,
        instance: EditorInstanceId,
        reply: Reply<T>,
    ) -> Answering<T> {
        let aborted = self.work(instance, reply.op()).flag();
        Answering {
            sender: self.services.sender.clone(),
            epoch: self.epoch,
            instance,
            op: reply.op(),
            aborted,
            answer: PhantomData,
        }
    }

    fn answer_now<T: Serialize>(&mut self, reply: Reply<T>, answer: T) {
        self.services
            .answers
            .push((reply.op(), Outcome::returned(&answer)));
    }

    fn refuse(&mut self, op: Option<OpId>, error: OpError) {
        if let Some(op) = op {
            self.services.answers.push((op, Outcome::Err(error)));
        }
    }

    fn scoped(
        &self,
        instance: EditorInstanceId,
        inward: bool,
        ids: &mut dyn FnMut(&mut Visit<'_>),
    ) {
        let scope = self.entries.get(&instance).and_then(|entry| {
            let block = entry.role.block()?.id;
            let source = revisions::source(block).unwrap_or(block);
            crate::be::with_graph(|graph| graph.scope_of(source)).flatten()
        });
        let Some(scope) = scope else {
            return;
        };
        crate::be::with_graph(|graph| {
            ids(&mut |id| {
                let block = Uuid::from_bytes(*id);
                let translated = match inward {
                    true => graph.to_real(scope, block, BlockIdRole::Existing),
                    false => graph.to_local(scope, block),
                };
                *id = translated.into_bytes();
            });
        });
    }

    pub(super) fn local(&self, instance: EditorInstanceId, mut id: [u8; 16]) -> [u8; 16] {
        self.scoped(instance, false, &mut |visit| visit(&mut id));
        id
    }

    pub(in crate::plugin_host) fn serve(&mut self, service: Service) {
        let Service {
            instance,
            op,
            mut call,
        } = service;
        self.scoped(instance, true, &mut |visit| {
            call.visit_block_ids(&mut |id| visit(id))
        });
        let previewing = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.role.block())
            .is_some_and(|block| revisions::source(block.id).is_some());
        if previewing
            && !matches!(
                call,
                Call::Fetch { .. } | Call::ReadData { .. } | Call::ListData { .. }
            )
        {
            return self.refuse(op, OpError::Denied);
        }
        if !self.entries.contains_key(&instance) {
            return self.refuse(op, OpError::Canceled);
        }
        match call {
            Call::ListRevisions { reply, block } => {
                let answering = self.answering(instance, reply);
                revisions::request(
                    Uuid::from_bytes(block),
                    revisions::Asked::List,
                    move |listed| {
                        if let revisions::Answered::Listed(listed) = listed {
                            answering.send_or_fail(listed);
                        }
                    },
                );
            }
            Call::PreviewRevision {
                reply,
                block,
                commit,
            } => {
                let answering = self.answering(instance, reply);
                let asked = revisions::Asked::Preview(commit);
                revisions::request(Uuid::from_bytes(block), asked, move |previewed| {
                    if let revisions::Answered::Previewed(previewed) = previewed {
                        answering.previewed(previewed);
                    }
                });
            }
            Call::RestoreRevision {
                reply,
                block,
                commit,
            } => {
                let answering = self.answering(instance, reply);
                let asked = revisions::Asked::Restore(commit);
                revisions::request(Uuid::from_bytes(block), asked, move |restored| {
                    if let revisions::Answered::Restored(restored) = restored {
                        answering.send_or_fail(restored);
                    }
                });
            }
            Call::ReleaseRevision { preview } => {
                let preview = Uuid::from_bytes(preview);
                if let Some(entry) = self.entries.get_mut(&instance)
                    && entry.revision_previews.remove(&preview)
                {
                    revisions::release(preview);
                }
            }
            Call::PickFile { reply, filter } => {
                let answering = self.answering(instance, reply);
                host::pick_file(filter, move |picked| answering.send(picked));
            }
            Call::SaveFile { reply, file } => {
                let answering = self.answering(instance, reply);
                host::save_file(file, move |saved| answering.send(saved));
            }
            Call::ExportBlock { reply, block } => {
                let block = Uuid::from_bytes(block);
                if !self.can_view(block) {
                    let refused = FileSave::Failed("You can't view this block.".to_owned());
                    return self.answer_now(reply, refused);
                }
                let answering = self.answering(instance, reply);
                crate::be::export(block, move |exported| match exported {
                    Ok(_) if answering.withdrawn() => {}
                    Ok((name, data)) => {
                        let file = SavedFile {
                            name,
                            mime_type: be_client::transfer::MIME_TYPE.to_owned(),
                            data,
                        };
                        host::save_file(file, move |saved| answering.send(saved));
                    }
                    Err(error) => answering.send(FileSave::Failed(error)),
                });
            }
            Call::ReadPlainFile { reply, block } => {
                let block = Uuid::from_bytes(block);
                if !self.can_view(block) {
                    return self.answer_now(reply, Err(Failure::Denied));
                }
                let answering = self.answering(instance, reply);
                crate::be::plain_file(block, move |read| {
                    answering.send_or_fail(read.map(|file| SavedFile {
                        name: file.name.unwrap_or_default(),
                        mime_type: file.media_type,
                        data: file.bytes,
                    }));
                });
            }
            Call::ImportBlocks { reply, parent } => {
                let parent = super::super::graph::parent_of(parent);
                if parent.block().is_some_and(|parent| !self.editable(parent)) {
                    return self.answer_now(reply, Err(Failure::Denied));
                }
                let answering = self.answering(instance, reply);
                let imported = |imported: Result<(Uuid, Uuid), String>| {
                    imported.map(|(block, block_type)| {
                        Some(ImportedBlock {
                            block_id: block.into_bytes(),
                            block_type: block_type.into_bytes(),
                        })
                    })
                };
                host::pick_file(beui::FileFilter::default(), move |picked| {
                    if answering.withdrawn() {
                        return;
                    }
                    match picked {
                        Ok(Some(file))
                            if be_block::plain::extension(&file.name)
                                == be_client::transfer::EXTENSION =>
                        {
                            answering.imported(imported(crate::be::import(&file.data, parent)));
                        }
                        Ok(Some(file)) => {
                            crate::be::import_file(file.name, file.data, parent, move |file| {
                                if !answering.withdrawn() {
                                    answering.imported(imported(file));
                                }
                            });
                        }
                        Ok(None) => answering.imported(Ok(None)),
                        Err(error) => answering.imported(Err(error)),
                    }
                });
            }
            Call::CopyBlocks {
                reply,
                source,
                parent,
            } => {
                let source = Uuid::from_bytes(source);
                let parent = super::super::graph::parent_of(parent);
                if !self.can_view(source)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    return self.answer_now(reply, Err(Failure::Denied));
                }
                let answering = self.answering(instance, reply);
                crate::be::export(source, move |exported| {
                    if answering.withdrawn() {
                        return;
                    }
                    answering.imported(exported.and_then(|(_, bytes)| {
                        crate::be::import(&bytes, parent).map(|(block, block_type)| {
                            Some(ImportedBlock {
                                block_id: block.into_bytes(),
                                block_type: block_type.into_bytes(),
                            })
                        })
                    }));
                });
            }
            Call::ListAccess { reply, block } => {
                let answering = self.answering(instance, reply);
                crate::be::list_access(Uuid::from_bytes(block), move |listed| {
                    answering.listed(super::super::graph::grants_of(listed));
                });
            }
            Call::SetAccess {
                block,
                account,
                access,
            } => crate::be::set_access(
                Uuid::from_bytes(block),
                Uuid::from_bytes(account),
                super::super::graph::access_of(access),
            ),
            Call::MakeContactLink {
                reply,
                block,
                access,
            } => {
                let answering = self.answering(instance, reply);
                let offer = (
                    Uuid::from_bytes(block),
                    super::super::graph::access_of(access),
                );
                crate::be::make_contact_link(Some(offer), move |made| answering.send_or_fail(made));
            }
            Call::Fetch { reply, url } => {
                if !allowed(&url, &self.network) {
                    return self.answer_now(reply, Err(Failure::Denied));
                }
                let op = reply.op();
                let answering = self.answering(instance, reply);
                let aborted = self.services.serving[&op].aborted.clone();
                http::fetch(url, Vec::new(), aborted, move |body| {
                    answering.send_or_fail(body)
                });
            }
            Call::ListData { reply } => {
                let answering = self.answering(instance, reply);
                discovery::data_listing(&self.plugin_id, move |index| {
                    answering.send_or_fail(index.and_then(|index| {
                        serde_json::from_slice(&index)
                            .map_err(|error| format!("the data index is {error}"))
                    }));
                });
            }
            Call::ReadData { reply, path } => {
                let answering = self.answering(instance, reply);
                discovery::data(&self.plugin_id, &path, move |body| {
                    answering.send_or_fail(body)
                });
            }
        }
    }

    pub(in crate::plugin_host) fn withdraw(&mut self, op: OpId) {
        let Some(work) = self.services.serving.remove(&op) else {
            return;
        };
        work.aborted.abort();
    }

    pub(super) fn withdraw_all(&mut self, instance: EditorInstanceId) {
        let ops: Vec<OpId> = self
            .services
            .serving
            .iter()
            .filter(|(_, work)| work.instance == instance)
            .map(|(op, _)| *op)
            .collect();
        for op in ops {
            self.withdraw(op);
        }
    }

    pub(super) fn forget_services(&mut self) {
        for (_, work) in self.services.serving.drain() {
            work.aborted.abort();
        }
        self.services.answers.clear();
    }

    pub(super) fn deliver_answers(&mut self) {
        while let Ok(delivered) = self.services.receiver.try_recv() {
            let live = delivered.epoch == self.epoch
                && self.entries.contains_key(&delivered.instance)
                && self
                    .services
                    .serving
                    .get(&delivered.op)
                    .is_some_and(|work| work.instance == delivered.instance);
            if live {
                self.services.serving.remove(&delivered.op);
            }
            let instance = delivered.instance;
            let outcome = match delivered.answer {
                Answer::Done(outcome) => outcome,
                Answer::Previewed(Ok(preview)) if !live => {
                    revisions::release(preview);
                    continue;
                }
                Answer::Previewed(previewed) => {
                    if let (Ok(preview), Some(entry)) =
                        (&previewed, self.entries.get_mut(&instance))
                    {
                        entry.revision_previews.insert(*preview);
                    }
                    let previewed: Result<[u8; 16], Failure> =
                        previewed.map(|preview| self.local(instance, preview.into_bytes()));
                    Outcome::returned(&previewed)
                }
                Answer::Imported(imported) => {
                    let imported = imported.map(|imported| {
                        imported.map(|imported| ImportedBlock {
                            block_id: self.local(instance, imported.block_id),
                            ..imported
                        })
                    });
                    Outcome::returned(&imported)
                }
                Answer::Listed(listed) => {
                    let listed = listed.map(|mut listed| {
                        listed.visit_block_ids(&mut |_, id| *id = self.local(instance, *id));
                        listed
                    });
                    Outcome::returned(&listed)
                }
            };
            if live {
                self.services.answers.push((delivered.op, outcome));
            }
        }
    }

    pub(in crate::plugin_host) fn take_answers(&mut self) -> Vec<(OpId, Outcome)> {
        self.deliver_answers();
        std::mem::take(&mut self.services.answers)
    }
}
