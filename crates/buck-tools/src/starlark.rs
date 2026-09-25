use serde_json::Value;

pub(crate) fn render(value: &Value, depth: usize) -> String {
    let indent = "    ".repeat(depth + 1);
    let closing = "    ".repeat(depth);
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => quote(text),
        Value::Array(items) if items.is_empty() => "[]".to_owned(),
        Value::Array(items) => {
            let body: String = items
                .iter()
                .map(|item| format!("{indent}{},\n", render(item, depth + 1)))
                .collect();
            format!("[\n{body}{closing}]")
        }
        Value::Object(fields) if fields.is_empty() => "{}".to_owned(),
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let body: String = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{indent}{}: {},\n",
                        quote(key),
                        render(&fields[key], depth + 1)
                    )
                })
                .collect();
            format!("{{\n{body}{closing}}}")
        }
    }
}

pub(crate) fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("a string always serializes")
}
