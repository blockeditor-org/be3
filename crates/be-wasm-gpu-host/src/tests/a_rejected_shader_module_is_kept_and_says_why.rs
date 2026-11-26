use super::web::*;
use super::*;

#[test]
fn a_rejected_shader_module_is_kept_and_says_why() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let mut made = None;
    let rejected = scoped(&mut gpu, device, |gpu| {
        made = Some(module(gpu, device, "fn main() {\n    let x = ;\n}\n"));
    });
    assert!(rejected.is_some(), "the rejection is raised");
    gpu.gpu_shader_module_get_compilation_info(made.unwrap(), be_wasm_ops::local::Promise::new(5));
    let info: GpuCompilationInfo = settled(&mut gpu, 5);
    let messages = gpu.gpu_compilation_info_messages(info);
    let [message] = messages.as_slice() else {
        panic!("one message says what is wrong, not {messages:?}");
    };
    assert_eq!(
        gpu.gpu_compilation_message_type(GpuCompilationMessage(message.0)),
        GpuCompilationMessageType::Error
    );
    assert_eq!(
        gpu.gpu_compilation_message_line_num(GpuCompilationMessage(message.0)),
        2
    );
}
