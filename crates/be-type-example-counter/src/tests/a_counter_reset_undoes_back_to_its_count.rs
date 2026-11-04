use super::*;

#[test]
fn a_counter_reset_undoes_back_to_its_count() {
    let counter = edited(
        &CounterContent::default(),
        [Counter::add(5), Counter::add(2)],
    );
    let reset = counter.root().reset();
    let step = counter.step(&reset).expect("a reset changes the count");
    let counter = edited(&counter, [reset]);
    assert_eq!(counter.root().value(), 0);

    let counter = edited(&counter, [Counter::add(1)]);
    let counter = edited(&counter, counter.revert(&step));

    assert_eq!(counter.root().value(), 8);
    assert_eq!(CounterContent::decode(&counter.encode()), Ok(counter));
}
