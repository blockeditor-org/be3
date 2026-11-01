use stable_hash::HashMap;

use block_plugin_api::{
    AccessListing, BlockIdRole, EditorInstanceId, Failure, FileSave, ImportedBlock, SavedFile,
    embed::host_services::Call,
    ops::{OpError, OpId, Outcome, Reply, wire},
};
use uuid::Uuid;

use super::{Instances, allowed};
use crate::wire::Service;
use block_app_base::{
    host,
    platform::http::{self, AbortFlag, Aborted},
};
use block_app_stack::{discovery, revisions};

type Visit<'a> = dyn FnMut(&mut [u8; 16]) + 'a;

pub(super) struct Delivered {
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

#[derive(Default)]
pub(super) struct Services {
    serving: HashMap<OpId, Work>,
    pub(super) answers: Vec<(OpId, Outcome)>,
}

struct Answering<T, E = wire::Plain> {
    sender: host::Post<Delivered>,
    epoch: u64,
    instance: EditorInstanceId,
    reply: Reply<T, E>,
    aborted: AbortFlag,
}

impl<T, E> Answering<T, E> {
    fn withdrawn(&self) -> bool {
        self.aborted.aborted()
    }

    fn deliver(self, answer: Answer) {
        self.sender.send(Delivered {
            epoch: self.epoch,
            instance: self.instance,
            op: self.reply.into_op(),
            answer,
        });
    }
}

impl<T, E: wire::Encode<T>> Answering<T, E> {
    fn send(self, answer: T) {
        if !self.withdrawn() {
            let outcome = Outcome::Ok(wire::encode::<E, T>(&answer));
            self.deliver(Answer::Done(outcome));
        }
    }
}

impl<T, E: wire::Encode<Result<T, Failure>>> Answering<Result<T, Failure>, E> {
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

    fn answering<T, E: wire::Encode<T>>(
        &mut self,
        instance: EditorInstanceId,
        reply: Reply<T, E>,
    ) -> Answering<T, E> {
        let aborted = self.work(instance, reply.op()).flag();
        Answering {
            sender: self.mail.answers.1.clone(),
            epoch: self.epoch,
            instance,
            reply,
            aborted,
        }
    }

    fn answer_now<T, E: wire::Encode<T>>(&mut self, reply: Reply<T, E>, answer: T) {
        self.services.answers.push(reply.answer(&answer));
    }

    fn refuse(&mut self, call: Call, error: OpError) {
        self.services.answers.push(call.refuse(error));
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
            block_app_stack::with_graph(|graph| graph.scope_of(source)).flatten()
        });
        let Some(scope) = scope else {
            return;
        };
        block_app_stack::with_graph(|graph| {
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

    pub fn serve(&mut self, service: Service) {
        let Service { instance, mut call } = service;
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
            return self.refuse(call, OpError::Denied);
        }
        if !self.entries.contains_key(&instance) {
            return self.refuse(call, OpError::Canceled);
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
            Call::ReleaseRevision { reply, preview } => {
                self.services.answers.push(reply.answer(&()));
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
                block_app_stack::export(block, move |exported| match exported {
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
                block_app_stack::plain_file(block, move |read| {
                    answering.send_or_fail(read.map(|file| SavedFile {
                        name: file.name.unwrap_or_default(),
                        mime_type: file.media_type,
                        data: file.bytes,
                    }));
                });
            }
            Call::ImportBlocks { reply, parent } => {
                let parent = block_app_stack::plugin_graph::parent_of(parent);
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
                            answering
                                .imported(imported(block_app_stack::import(&file.data, parent)));
                        }
                        Ok(Some(file)) => {
                            block_app_stack::import_file(
                                file.name,
                                file.data,
                                parent,
                                move |file| {
                                    if !answering.withdrawn() {
                                        answering.imported(imported(file));
                                    }
                                },
                            );
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
                let parent = block_app_stack::plugin_graph::parent_of(parent);
                if !self.can_view(source)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    return self.answer_now(reply, Err(Failure::Denied));
                }
                let answering = self.answering(instance, reply);
                block_app_stack::export(source, move |exported| {
                    if answering.withdrawn() {
                        return;
                    }
                    answering.imported(exported.and_then(|(_, bytes)| {
                        block_app_stack::import(&bytes, parent).map(|(block, block_type)| {
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
                block_app_stack::list_access(Uuid::from_bytes(block), move |listed| {
                    answering.listed(block_app_stack::plugin_graph::grants_of(listed));
                });
            }
            Call::SetAccess {
                reply,
                block,
                account,
                access,
            } => {
                self.services.answers.push(reply.answer(&()));
                block_app_stack::set_access(
                    Uuid::from_bytes(block),
                    Uuid::from_bytes(account),
                    block_app_stack::plugin_graph::access_of(access),
                );
            }
            Call::MakeContactLink {
                reply,
                block,
                access,
            } => {
                let answering = self.answering(instance, reply);
                let offer = (
                    Uuid::from_bytes(block),
                    block_app_stack::plugin_graph::access_of(access),
                );
                block_app_stack::make_contact_link(Some(offer), move |made| {
                    answering.send_or_fail(made)
                });
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

    pub fn withdraw(&mut self, op: OpId) {
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

    pub(super) fn service_answered(&mut self, delivered: Delivered) {
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
                return;
            }
            Answer::Previewed(previewed) => {
                if let (Ok(preview), Some(entry)) = (&previewed, self.entries.get_mut(&instance)) {
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

    pub fn take_answers(&mut self) -> Vec<(OpId, Outcome)> {
        let desk = self.mail.desk;
        desk.deliver(self);
        std::mem::take(&mut self.services.answers)
    }
}
