use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::shader::ShaderModule;

pub fn load_shader_module(
    context: &VulkanContext,
    content: &[u8],
    kind: shaderc::ShaderKind,
) -> Result<Arc<ShaderModule>> {
    let source = std::str::from_utf8(content)?;
    let header = std::fs::read_to_string("app/header.glsl")?;
    let combined = format!("{header}\n{source}");

    let compiler = shaderc::Compiler::new().unwrap();

    // let input_file_name = path.to_str().ok_or(anyhow!("Invalid file name"))?;
    let spirv = compiler.compile_into_spirv(&combined, kind, "input_file_name", "main", None)?;
    let spirv_binary = spirv.as_binary_u8();

    let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
    let ep = &reflect.enumerate_entry_points().unwrap()[0];
    println!("SPIRV Metadata: {:#?}", ep);

    // println!("Shader '{path:?}' SPIRV size: {}", spirv_binary.len());

    let module =
        unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) };

    Ok(module?)
}
