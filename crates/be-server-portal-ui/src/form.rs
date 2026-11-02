use beui::reactive::*;
use beui::styled::*;
use beui::{ItemSize, NodeId};

pub fn fixed(text: String) -> Memo<String> {
    create_memo(move || text.clone())
}

#[component]
pub fn PortalForm(
    action: String,
    csrf: String,
    #[prop(default = None)] code: Option<String>,
    #[prop(default = Vec::new())] hidden: Vec<(&'static str, String)>,
    children: Children<ListChild>,
) -> NodeId {
    let mut fields = vec![("csrf", csrf)];
    fields.extend(code.map(|code| ("code", code)));
    fields.extend(hidden);
    view! {
        <Form action hidden={fields}>
            <List spacing=Space::Lg>{children}</List>
        </Form>
    }
}

#[component]
pub fn Field(
    kind: InputKind,
    name: String,
    label: String,
    #[prop(default = String::new())] autocomplete: String,
    #[prop(default = String::new())] hint: String,
    #[prop(default = String::new())] problem: String,
    value: Signal<String>,
) -> NodeId {
    let first = value.get_untracked();
    let shown = create_memo(move || match value.with(|value| *value == first) {
        true => problem.clone(),
        false => String::new(),
    });
    let has_problem = create_memo(move || !shown.get().is_empty());
    let has_hint = !hint.is_empty();
    let hint = fixed(hint);
    let label = fixed(label);
    view! {
        <List spacing=Space::Xs>
            <Text size=TextSize::Sm weight=Weight::Bold>{label}</Text>
            <TextInput
                value
                label={label}
                kind
                name
                autocomplete
                @sizing=ItemSize::Percent(100.0)
            />
            <Show condition={has_problem}>
                <Text size=TextSize::Sm color=TextColor::Danger>{shown}</Text>
            </Show>
            <Show condition={has_hint}>
                <Text style=TextStyle::CAPTION>{hint}</Text>
            </Show>
        </List>
    }
}

#[component]
pub fn Submit(
    label: String,
    #[prop(default = ButtonVariant::Primary)] variant: ButtonVariant,
    #[prop(default = true)] wide: bool,
) -> NodeId {
    view! {
        <Button label variant wide submit=true />
    }
}

#[component]
pub fn PageLink(label: String, href: String) -> NodeId {
    view! {
        <Link label href={Some(href)} />
    }
}
