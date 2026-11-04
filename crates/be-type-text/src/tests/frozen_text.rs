pub const REGISTRY: &str = r#"
format 1
default text.indentation.spaces.width = 2
default text.language = "markdown"
kind text
property text.body: text
property text.indentation: register
property text.language: register
"#;

pub const DOCUMENT: &str = "
    a264726f6f74a464626f6479582b666e206d61696e2829207b0a097072696e74
    6c6e21282268c3a96c6c6f2c20e4b896e7958c22293b0a7d0a65246b696e6464
    74657874686c616e677561676564727573746b696e64656e746174696f6e6474
    61627366666f726d617401
";
