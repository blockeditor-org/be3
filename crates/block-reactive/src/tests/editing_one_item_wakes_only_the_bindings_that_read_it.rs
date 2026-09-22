use super::*;

#[test]
fn editing_one_item_wakes_only_the_bindings_that_read_it() {
    let client = client();
    let block = client.create_block(Calendar::default());
    for title in ["write", "review", "ship"] {
        block.operate(CalendarOperation::AddEvent {
            event: CalendarEvent::new(title.to_owned(), 0, 60),
        });
    }
    let ids: Vec<Uuid> = block
        .read()
        .unwrap()
        .events()
        .iter()
        .map(|event| event.id)
        .collect();

    let source = BlockSource::new(block.clone(), || {});
    let items = source.project_keyed(|calendar, items| {
        items.reconcile(calendar.events().iter().map(|event| (event.id, event)));
    });
    let done = source.project(|calendar: &Calendar| calendar.events().len());

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

    let mut renamed = block.read().unwrap().event(ids[1]).unwrap().clone();
    renamed.title = "review again".to_owned();
    block.operate(CalendarOperation::UpdateEvent { event: renamed });
    source.pump();

    assert_eq!(
        runs.iter().map(|run| run.get()).collect::<Vec<_>>(),
        [0, 1, 0]
    );
    assert_eq!(done_runs.get(), 0);
}
