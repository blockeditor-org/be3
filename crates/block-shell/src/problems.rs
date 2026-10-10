use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{Memo, NodeRef, component, create_memo, view};
use block_editor_beui::beui::styled::{Toast, Toasts};
use block_editor_beui::{Editor, ProblemAction, Problems};

pub fn problem_toasts(editor: &Editor) -> Memo<Vec<Toast>> {
    let problems = editor.host_value::<Problems>();
    create_memo(move || {
        problems.with(|problems| {
            problems
                .iter()
                .map(|problem| Toast {
                    id: problem.id,
                    message: problem.message.clone(),
                    danger: true,
                    ..Toast::default()
                })
                .collect()
        })
    })
}

#[component]
pub fn ProblemToasts(editor: Editor, anchor: NodeRef) -> NodeId {
    let toasts = problem_toasts(&editor);
    view! {
        <Toasts
            anchor
            toasts={toasts}
            on_dismiss={move |id: u64| editor.act(ProblemAction::Dismiss(id))}
            on_action={|_: (u64, String)| {}}
            on_activate={|_: u64| {}}
        />
    }
}
