use super::*;

fn compiled(module: &wgpu::ShaderModule) -> wgpu::CompilationInfo {
    match pin!(module.get_compilation_info()).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(info) => info,
        Poll::Pending => panic!("a native host has the compilation info at once"),
    }
}

#[test]
fn a_shader_module_says_where_its_wgsl_went_wrong() {
    let (device, _queue) = gpu();
    let fine = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(FILL.into()),
    });
    let messages = compiled(&fine).messages;
    assert!(messages.is_empty(), "{messages:?}");
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let broken = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl("@compute @workgroup_size(1)\nfn main() { let x = ; }\n".into()),
    });
    refusal(scope);
    let messages = compiled(&broken).messages;
    let [message] = messages.as_slice() else {
        panic!("one message says what is wrong, not {messages:?}");
    };
    assert_eq!(message.message_type, wgpu::CompilationMessageType::Error);
    let location = message.location.expect("the message points into the wgsl");
    assert_eq!(location.line_number, 2, "{message:?}");
}
