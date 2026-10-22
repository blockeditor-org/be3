use std::cell::RefCell;
use std::rc::Rc;

use reactive::{Get, Scope, Set, create_effect, create_memo, create_signal, flush};

#[derive(Default)]
struct Label {
    text: String,
}

fn main() {
    let scope = Scope::new();
    let label = Rc::new(RefCell::new(Label::default()));
    let count = scope.run(|| create_signal(0));

    scope.run(|| {
        let text = create_memo(move || format!("Count: {}", count.get()));
        let label = label.clone();
        create_effect(move || label.borrow_mut().text = text.get());
    });

    {
        let _label = label.borrow_mut();
        count.set(1);
        count.update(|count| *count += 1);
    }
    flush();
    assert_eq!(label.borrow().text, "Count: 2");

    scope.dispose();
    count.set(3);
    flush();
    assert_eq!(label.borrow().text, "Count: 2");
    println!("{}", label.borrow().text);
}
