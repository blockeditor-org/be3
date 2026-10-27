use beui_macros::{component, view};
use beui_view::primitives::*;
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

pub(crate) fn cell_styles() -> Vec<(&'static str, Prop<Option<CssValue>>)> {
    vec![
        ("display", Prop::Static(Some(CssValue::Keyword("grid")))),
        (
            "grid-template",
            Prop::Static(Some(CssValue::Keyword("minmax(0, 1fr) / minmax(0, 1fr)"))),
        ),
    ]
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
    let fields: Vec<NodeId> = hidden
        .into_iter()
        .map(|(name, value)| {
            view! {
                <Element
                    tag="input"
                    attributes={vec![
                        ("type", Prop::Static(Some("hidden".to_owned()))),
                        ("name", Prop::Static(Some(name.to_owned()))),
                        ("value", Prop::Static(Some(value))),
                    ]}
                />
            }
        })
        .collect();
    let body = view! {
        <List spacing=0.0>{Children::<ListChild>::from_iter(fields)}{content}</List>
    };
    let action = Prop::dynamic(move || Some(action.get()).filter(|action| !action.is_empty()));
    view! {
        <Element
            tag="form"
            attributes={vec![
                ("method", Prop::Static(Some("post".to_owned()))),
                ("action", action),
                ("novalidate", Prop::Static(Some(String::new()))),
            ]}
            styles={cell_styles()}
            events={vec![(
                "submit",
                Callback::new(move |event: ElementEvent| {
                    if !on_submit.is_empty() {
                        event.prevent_default();
                        on_submit.call();
                    }
                }),
            )]}
        >
            {body}
        </Element>
    }
}
