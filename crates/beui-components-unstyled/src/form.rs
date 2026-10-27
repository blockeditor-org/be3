use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::escape::beui_html::{Event, Node, elements as html};
use beui_view::escape::{Escape, html_escape};
use beui_view::{CORE, Core};

#[derive(Clone, Copy)]
struct FormContext {
    submit: ClickCallback,
}

pub(crate) fn form_submit() -> Option<ClickCallback> {
    use_context::<FormContext>().map(|form| form.submit)
}

pub(crate) fn fill_cell(escape: &Escape) {
    escape.style("display", "grid".to_owned());
    escape.style("grid-template", "minmax(0, 1fr) / minmax(0, 1fr)".to_owned());
}

#[component]
pub fn Form(
    #[prop(default = String::new())] action: Prop<String>,
    #[prop(default = Vec::new())] hidden: Vec<(&'static str, String)>,
    on_submit: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    let submit = on_submit;
    provide_context(FormContext {
        submit: ClickCallback::new(move || submit.call()),
    });
    let content = content.call(());
    if CORE != Core::Html {
        return content;
    }
    let action = Prop::dynamic(move || Some(action.get()).filter(|action| !action.is_empty()));
    html_escape(move |escape| {
        fill_cell(escape);
        let content = escape.embed(content);
        let fields: Children<Node> = hidden
            .into_iter()
            .map(|(name, value)| {
                view! {
                    <html::Input r#type="hidden" name={name.to_owned()} value={value} />
                }
            })
            .collect();
        view! {
            <html::Form
                method="post"
                action={action}
                novalidate=true
                on_submit={move |event: Event| {
                    if !on_submit.is_empty() {
                        event.prevent_default();
                        on_submit.call();
                    }
                }}
            >
                {fields}
                {content}
            </html::Form>
        }
    })
}
