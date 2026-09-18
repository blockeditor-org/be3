use beui::reactive::{
    Button, Column, ForEach, KeyedStore, ReadSignal, Row, Show, Text, build, clone, component,
    create_memo, create_signal, view,
};
use beui::{App, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui counter", CounterApp::new())
}

#[component]
fn HistoryEntry(value: ReadSignal<i64>) -> NodeId {
    let text = create_memo(move || value.get().to_string());
    view! {
        <Text string={text} />
    }
}

#[component]
fn App() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let history: KeyedStore<u64, i64> = KeyedStore::new();
    let (entries, set_entries) = create_signal(Vec::<(u64, i64)>::new());
    let (next_id, set_next_id) = create_signal(0u64);
    let record = clone!(history entries -> move || {
        history.reconcile(entries.get_untracked().iter().map(|(id, value)| (*id, value)));
    });

    let is_zero = {
        let count = count.clone();
        create_memo(move || count.get() <= 0)
    };
    let is_nonzero = {
        let count = count.clone();
        create_memo(move || count.get() != 0)
    };

    let decrement_click = {
        let set_count = set_count.clone();
        let decrement_record = record.clone();
        let set_entries = set_entries.clone();
        let count = count.clone();
        let next_id = next_id.clone();
        let set_next_id = set_next_id.clone();
        move || {
            set_count.update(|value| *value -= 1);
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_entries.update(|entries| entries.push((id, count.get())));
            decrement_record();
        }
    };

    let increment_click = {
        let set_count = set_count.clone();
        let increment_record = record.clone();
        let set_entries = set_entries.clone();
        let count = count.clone();
        let next_id = next_id.clone();
        let set_next_id = set_next_id.clone();
        move || {
            set_count.update(|value| *value += 1);
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_entries.update(|entries| entries.push((id, count.get())));
            increment_record();
        }
    };

    let reset_click = move || {
        set_count.set(0);
        set_entries.set(Vec::new());
        record();
    };

    let count_text = create_memo(move || count.get().to_string());

    view! {
        <Column spacing=8.0>
            <Row spacing=8.0>
                <Button disabled={is_zero} on_click={decrement_click}>
                    <Text string="-" />
                </Button>
                <Text string={count_text} />
                <Button on_click={increment_click}>
                    <Text string="+" />
                </Button>
                <Show condition={is_nonzero}>
                    <Button on_click={reset_click}>
                        <Text string="reset" />
                    </Button>
                </Show>
            </Row>
            <Column spacing=4.0>
                <ForEach keys={history.keys()}>
                    {move |id: u64| {
                        let value = history.get(&id);
                        view! {
                            <HistoryEntry value />
                        }
                    }}
                </ForEach>
            </Column>
        </Column>
    }
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(|| {
                view! {
                    <App />
                }
            }),
        }
    }
}

impl App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        Color32::BLACK
    }
}
