use std::rc::Rc;

use beui::reactive::{Memo, ReadSignal, create_effect, create_memo, create_signal};
use uuid::Uuid;

use be_block::Root;
use be_block::be_model::{Document, Field, FieldRef, List, Model, ObjectId};
use block_editor_plugin::Objects;

use crate::{BlockQuery, ContentProjection, Editor};

pub fn content_settled(editor: Editor, block: Uuid, loaded: ReadSignal<bool>) -> Memo<bool> {
    let found = editor.blocks().watch(BlockQuery::Block(block));
    create_memo(move || {
        loaded.get()
            || (found.is_loaded()
                && found
                    .read()
                    .first()
                    .is_none_or(|info| !info.access.can_view()))
    })
}

pub struct RelatedContent<C> {
    editor: Editor,
    id: Memo<Option<Uuid>>,
    marker: std::marker::PhantomData<fn() -> C>,
}

impl<C> Clone for RelatedContent<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for RelatedContent<C> {}

impl<C> RelatedContent<C>
where
    C: be_block::LiveEdit + Clone + Default,
{
    pub(crate) fn new(editor: Editor, id: Memo<Option<Uuid>>) -> Self {
        Self {
            editor,
            id,
            marker: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> Option<Uuid> {
        self.id.get_untracked()
    }

    pub fn projection(&self) -> Option<ContentProjection<C>> {
        self.id().map(|id| self.editor.content_of::<C>(id))
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Option<T> {
        self.projection()?.read(read)
    }

    pub fn settled(&self) -> Memo<bool> {
        let (settled, set_settled) = create_signal(false);
        let id = self.id;
        let editor = self.editor;
        create_effect(move || {
            let Some(block) = id.get() else {
                set_settled.set(false);
                return;
            };
            let loaded = editor.content_of::<C>(block).loaded();
            let held = content_settled(editor, block, loaded);
            create_effect(move || set_settled.set(held.get()));
        });
        create_memo(move || settled.get())
    }

    pub fn operate(&self, operation: C::Op) {
        if let Some(projection) = self.projection() {
            projection.operate(operation);
        }
    }

    pub fn project<T>(&self, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + Default + PartialEq + 'static,
    {
        let project = Rc::new(project);
        let (read, write) = create_signal(T::default());
        let id = self.id;
        let editor = self.editor;
        create_effect(move || {
            let Some(block) = id.get() else {
                write.set(T::default());
                return;
            };
            let project = Rc::clone(&project);
            let inner = editor
                .content_of::<C>(block)
                .project(move |content| project(content));
            create_effect(move || write.set(inner.get()));
        });
        read
    }
}

impl<R: Root + 'static> RelatedContent<Document<R>>
where
    Document<R>: be_block::LiveEdit + Clone + Default,
{
    pub fn objects<M: 'static, T: Model + 'static>(
        &self,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
    ) -> ReadSignal<Objects<T>> {
        let (read, write) = create_signal(Objects::default());
        let id = self.id;
        let editor = self.editor;
        create_effect(move || {
            let Some(block) = id.get() else {
                write.set(Objects::default());
                return;
            };
            let inner = editor
                .content_of::<Document<R>>(block)
                .objects(owner, field);
            create_effect(move || write.set(inner.get()));
        });
        read
    }

    pub fn field<M: 'static, F>(&self, object: ObjectId, field: FieldRef<M, F>) -> ReadSignal<F>
    where
        F: Field + Clone + Default + PartialEq + 'static,
    {
        let (read, write) = create_signal(F::default());
        let id = self.id;
        let editor = self.editor;
        create_effect(move || {
            let Some(block) = id.get() else {
                write.set(F::default());
                return;
            };
            let inner = editor
                .content_of::<Document<R>>(block)
                .field(object, field);
            create_effect(move || write.set(inner.get()));
        });
        read
    }
}
