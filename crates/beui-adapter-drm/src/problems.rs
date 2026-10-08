use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Inner {
    listener: Option<Box<dyn Fn(String)>>,
    held: Vec<String>,
}

#[derive(Clone, Default)]
pub struct Problems(Rc<RefCell<Inner>>);

impl Problems {
    pub fn listen(&self, listener: impl Fn(String) + 'static) {
        let held = std::mem::take(&mut self.0.borrow_mut().held);
        for problem in held {
            listener(problem);
        }
        self.0.borrow_mut().listener = Some(Box::new(listener));
    }

    pub(crate) fn report(&self, problem: String) {
        eprintln!("beui: {problem}");
        let inner = self.0.borrow();
        match &inner.listener {
            Some(listener) => listener(problem),
            None => {
                drop(inner);
                self.0.borrow_mut().held.push(problem);
            }
        }
    }
}
