use super::*;

#[test]
fn a_rejected_shader_module_is_kept_and_says_why() {
    let mut gpu = gpu();
    let module = gpu
        .create_shader_module(abi::ShaderModuleDescriptor {
            label: None,
            wgsl: "fn main() {\n    let x = ;\n}\n".into(),
        })
        .expect("a rejected module is still made");
    assert!(gpu.take_error().is_some(), "the rejection is reported");
    let messages = gpu
        .shader_module_get_compilation_info(module)
        .expect("a native device has the messages at once");
    let [message] = messages.as_slice() else {
        panic!("one message says what is wrong, not {messages:?}");
    };
    assert_eq!(message.message_type, abi::CompilationMessageType::Error);
    assert_eq!(message.location.as_ref().map(|at| at.line_number), Some(2));
}
