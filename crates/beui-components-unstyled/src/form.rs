use beui_macros::{component, view};
use beui_view::escape::beui_html::{Event, Node, attr, elements as html, flag};
use beui_view::escape::{Escape, html_escape};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
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
    escape.style(
        "grid-template",
        "minmax(0, 1fr) / minmax(0, 1fr)".to_owned(),
    );
}

/// On the HTML core, `content` with a hidden input that posts `value` under `name`
/// to the `Form` around it, and nothing while `value` is `None` (an unchecked box) or
/// the field is `disabled`, as HTML posts no disabled control; natively, or without a
/// `name`, just `content`. For a field beui draws itself, which has no `<input>` of its
/// own to post. With a `name` the wrapper is the field's node on HTML, so a test id or
/// node ref given to the field lands on it.
#[component]
pub(crate) fn Posted(
    name: Prop<String>,
    value: Memo<Option<String>>,
    disabled: Memo<bool>,
    content: NodeId,
) -> NodeId {
    if CORE != Core::Html || matches!(&name, Prop::Static(name) if name.is_empty()) {
        return content;
    }
    html_escape(move |escape| {
        fill_cell(escape);
        let content = escape.embed(content);
        let name = Prop::dynamic(move || Some(name.get()).filter(|name| !name.is_empty()));
        view! {
            <html::Div>
                <html::Input
                    r#type="hidden"
                    name={name}
                    value={Prop::dynamic(move || value.get())}
                    disabled={create_memo(move || disabled.get() || value.get().is_none())}
                />
                {content}
            </html::Div>
        }
    })
}

/// On the HTML core a `<form method="post" novalidate>` with the `hidden` inputs,
/// so a server-rendered page posts before wasm loads, and `on_submit` once it
/// runs. Natively it is no node: Enter in a field inside it and its submit buttons
/// call `on_submit`.
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
    let action = match action {
        Prop::Static(action) => Prop::Static(Some(action).filter(|action| !action.is_empty())),
        action => Prop::dynamic(move || Some(action.get()).filter(|action| !action.is_empty())),
    };
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
                @attr={("method", "post".to_owned())}
                @attr={("action", action)}
                @flag={("novalidate", true)}
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
