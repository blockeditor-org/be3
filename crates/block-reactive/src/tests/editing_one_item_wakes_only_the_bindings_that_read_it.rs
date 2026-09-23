use super::*;

#[test]
fn editing_one_item_wakes_only_the_bindings_that_read_it() {
    let client = client();
    let block = client.create_block(Deck::default());
    for index in 0..3 {
        block.operate(add_card(index));
    }
    let ids: Vec<Uuid> = block
        .read()
        .unwrap()
        .cards()
        .iter()
        .map(|card| card.id)
        .collect();

    let source = BlockSource::new(block.clone(), || {});
    let items = source.project_keyed(|deck, items| {
        items.reconcile(deck.cards().iter().map(|card| (card.id, card)));
    });
    let done = source.project(|deck: &Deck| deck.cards().len());

    let scope = Scope::new();
    let runs: Vec<Rc<Cell<usize>>> = ids.iter().map(|_| Rc::new(Cell::new(0))).collect();
    let done_runs = Rc::new(Cell::new(0));
    scope.run(|| {
        for (id, runs) in ids.iter().zip(&runs) {
            let item = items.get(id);
            let runs = runs.clone();
            create_effect(move || {
                item.with(|_| ());
                runs.set(runs.get() + 1);
            });
        }
        let counted = done_runs.clone();
        create_effect(move || {
            done.with(|_| ());
            counted.set(counted.get() + 1);
        });
    });
    for run in &runs {
        run.set(0);
    }
    done_runs.set(0);

    block.operate(DeckOperation::Turn {
        id: ids[1],
        face: Uuid::new_v4(),
    });
    source.pump();

    assert_eq!(
        runs.iter().map(|run| run.get()).collect::<Vec<_>>(),
        [0, 1, 0]
    );
    assert_eq!(done_runs.get(), 0);
}
