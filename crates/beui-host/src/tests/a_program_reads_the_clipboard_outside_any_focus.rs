use super::*;

struct Reading {
    asks: Rc<RefCell<usize>>,
    read: Rc<RefCell<Vec<Option<String>>>>,
}

impl Program for Reading {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let mut asks = self.asks.borrow_mut();
        while *asks > 0 {
            *asks -= 1;
            let read = self.read.clone();
            context.read_clipboard(move |text| read.borrow_mut().push(text));
        }
    }
}

#[test]
fn a_program_reads_the_clipboard_outside_any_focus() {
    let asks = Rc::new(RefCell::new(1));
    let read = Rc::new(RefCell::new(Vec::new()));
    let mut driven = headless(Reading {
        asks: asks.clone(),
        read: read.clone(),
    });
    driven.ask(&["settle"]).expect("the program settles");
    assert_eq!(*read.borrow(), [None], "an empty clipboard reads as nothing");

    driven
        .ask(&["clipboard", "copied outside"])
        .expect("the clipboard is set");
    *asks.borrow_mut() = 1;
    driven.frame(Vec::new());
    driven.frame(Vec::new());
    assert_eq!(*read.borrow(), [None, Some("copied outside".to_owned())]);
}
