use std::cell::RefCell;
use std::rc::Rc;

use uuid::Uuid;

use crate::{BlockGraph, BlockParent, DialogAnswer, EditorHost};

type Maker = Rc<dyn Fn() -> Result<Uuid, String>>;

#[derive(Clone)]
pub struct CreationCore(Rc<Inner>);

struct Inner {
    host: EditorHost,
    template: String,
    maker: RefCell<Option<Maker>>,
}

impl CreationCore {
    pub const MAIN: &str = "main";

    pub fn new(host: EditorHost, template: impl Into<String>) -> Self {
        Self(Rc::new(Inner {
            host,
            template: template.into(),
            maker: RefCell::new(None),
        }))
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn graph(&self) -> BlockGraph {
        self.0.host.graph()
    }

    pub fn template(&self) -> &str {
        &self.0.template
    }

    pub fn create<C: be_block::BlockContent>(&self, content: &C) -> Uuid {
        self.graph().create(content, BlockParent::Detached)
    }

    pub fn seed_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.0.host.seed_content(block, content);
    }

    pub fn accept(&self) {
        self.0.host.close_dialog(DialogAnswer::Accepted);
    }

    pub fn cancel(&self) {
        self.0.host.close_dialog(DialogAnswer::Dismissed);
    }

    pub fn on_create(&self, make: impl Fn() -> Result<Uuid, String> + 'static) {
        *self.0.maker.borrow_mut() = Some(Rc::new(make));
    }

    pub fn create_block(&self) -> Result<Uuid, String> {
        let maker = self.0.maker.borrow().clone();
        match maker {
            Some(make) => make(),
            None => Err("this editor does not create blocks".to_owned()),
        }
    }
}
