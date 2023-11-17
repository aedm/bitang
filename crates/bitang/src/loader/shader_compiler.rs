use crate::control::controls::GlobalType;
use crate::loader::file_cache::{ContentHash, FileCache};
use crate::loader::ResourcePath;
use crate::render::shader::GlobalUniformMapping;
use crate::tool::VulkanContext;
use anyhow::{anyhow, bail, ensure, Context, Result};

use shaderc::{IncludeCallbackResult, IncludeType};
use spirv_reflect::types::{ReflectDescriptorType, ReflectTypeFlags};
use std::cell::RefCell;

use spirq::ty::ScalarType::Float;
use spirq::ty::{ScalarType, Type};
use spirq::{DescriptorType, ReflectConfig, Variable};
use std::mem::size_of;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, instrument, trace, warn};
use vulkano::shader::ShaderModule;

const GLOBAL_UNIFORM_PREFIX: &str = "g_";

#[derive(Debug)]
pub struct IncludeChainLink {
    pub resource_path: ResourcePath,
    pub hash: ContentHash,
}

pub struct ShaderCompilation {
    pub shader_artifact: ShaderArtifact,
    pub include_chain: Vec<IncludeChainLink>,
}

impl ShaderCompilation {
    #[instrument(skip(context, kind, source, file_hash_cache))]
    pub fn compile_shader(
        context: &Arc<VulkanContext>,
        path: &ResourcePath,
        source: &str,
        kind: shaderc::ShaderKind,
        file_hash_cache: Arc<FileCache>,
    ) -> Result<Self> {
        // let path_str = path.to_string();
        let now = std::time::Instant::now();

        // TODO: use this when implicit common.glsl include is deprecated
        // let source = {
        //     tokio::runtime::Handle::current()
        //         .block_on(async move { file_hash_cache.get(&path).await })
        // }?;

        let compiler = shaderc::Compiler::new().context("Failed to create shader compiler")?;
        let deps = RefCell::new(vec![]);
        let spirv = {
            let include_callback = |include_name: &str, include_type, source_name: &str, depth| {
                Self::include_callback(
                    include_name,
                    include_type,
                    source_name,
                    depth,
                    &mut deps.borrow_mut(),
                    &file_hash_cache,
                )
            };
            let mut options = shaderc::CompileOptions::new()
                .context("Failed to create shader compiler options")?;
            options.set_target_env(
                shaderc::TargetEnv::Vulkan,
                shaderc::EnvVersion::Vulkan1_2 as u32,
            );
            // TODO: Enable optimization
            // options.set_optimization_level(shaderc::OptimizationLevel::Performance);
            options.set_include_callback(include_callback);
            options.set_generate_debug_info();
            compiler.compile_into_spirv(source, kind, &path.to_string(), "main", Some(&options))?
        };
        info!("compiled in {:?}.", now.elapsed());

        let shader_artifact =
            ShaderArtifact::from_spirv_binary(context, kind, spirv.as_binary_u8())?;

        Ok(Self {
            shader_artifact,
            include_chain: deps.take(),
        })
    }

    fn include_callback(
        include_name: &str,
        include_type: IncludeType,
        source_name: &str,
        depth: usize,
        deps: &mut Vec<IncludeChainLink>,
        file_hash_cache: &FileCache,
    ) -> IncludeCallbackResult {
        trace!(
            "#include '{include_name}' ({include_type:?}) from '{source_name}' (depth: {depth})",
        );
        let source_path = ResourcePath::from_str(source_name).map_err(|err| err.to_string())?;
        let include_path = source_path.relative_path(include_name);
        let included_source_u8 = tokio::runtime::Handle::current()
            .block_on(async {
                // x
                let x = file_hash_cache.get(&include_path);
                x.await
            })
            .map_err(|err| err.to_string())?;
        deps.push(IncludeChainLink {
            resource_path: include_path.clone(),
            hash: included_source_u8.hash,
        });
        let content =
            String::from_utf8(included_source_u8.content.clone()).map_err(|err| err.to_string())?;
        Ok(shaderc::ResolvedInclude {
            resolved_name: include_path.to_string(),
            content,
        })
    }
}

/// A descriptor binding point for a named resource
#[derive(Debug)]
pub struct NamedResourceBinding {
    pub name: String,
    pub binding: u32,
}

/// Metadata of a local uniform extracted from the compiled shader
#[derive(Debug)]
pub struct ShaderCompilationLocalUniform {
    pub name: String,
    pub f32_offset: usize,
    pub f32_count: usize,
}
/// The compiled shader module and metadata.
#[derive(Debug)]
pub struct ShaderArtifact {
    pub module: Arc<ShaderModule>,
    pub samplers: Vec<NamedResourceBinding>,
    pub buffers: Vec<NamedResourceBinding>,
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,
    pub local_uniform_bindings: Vec<ShaderCompilationLocalUniform>,
    pub uniform_buffer_size: usize,
}

impl ShaderArtifact {
    fn from_spirv_binary(
        context: &Arc<VulkanContext>,
        kind: shaderc::ShaderKind,
        spirv_binary: &[u8],
    ) -> Result<Self> {
        // Extract metadata from SPIRV
        let entry_points = ReflectConfig::new()
            .spv(spirv_binary)
            // Set this true if you want to reflect all resources no matter it's
            // used by an entry point or not.
            .ref_all_rscs(false)
            // Combine sampled image and separated sampler states if they are bound
            // to the same binding point.
            .combine_img_samplers(true)
            // Generate unique names for types and struct fields to help further
            // processing of the reflection data. Otherwise, the debug names are
            // assigned.
            .gen_unique_names(false)
            // Specialize the constant at `SpecID=3` with unsigned integer 7. The
            // constants specialized here won't be listed in the result entry point's
            // variable list.
            // .specialize(3, ConstantValue::U32(7))
            // Do the work.
            .reflect()?;
        let entry_point = entry_points
            .iter()
            .find(|ep| ep.name == "main")
            .context("Failed to find entry point 'main'")?;

        let module = unsafe { ShaderModule::from_bytes(context.device.clone(), spirv_binary) }?;

        let descriptor_set_index = match kind {
            shaderc::ShaderKind::Vertex => 0,
            shaderc::ShaderKind::Fragment => 1,
            shaderc::ShaderKind::Compute => 0,
            _ => panic!("Unsupported shader kind"),
        };

        // Find the descriptor set that belongs to the current shader stage
        // let Some(descriptor_set) = entry_point
        //     .get_descriptor_sets()
        //     .map_err(|err| anyhow!("Failed to get descriptor sets: {err}"))?
        //     .get(&descriptor_set_index)
        // else {
        //     // The entire descriptor set is empty, so we can just use the module
        //     return Ok(ShaderArtifact {
        //         module,
        //         samplers: vec![],
        //         buffers: vec![],
        //         local_uniform_bindings: vec![],
        //         global_uniform_bindings: vec![],
        //         uniform_buffer_size: 0,
        //     });
        // };

        let mut samplers = Vec::new();
        let mut buffers = Vec::new();
        let mut global_uniform_bindings = Vec::new();
        let mut local_uniform_bindings = Vec::new();
        let mut uniform_buffer_size = 0;

        for var in &entry_point.vars {
            match var {
                Variable::Descriptor {
                    desc_ty,
                    ty,
                    name,
                    desc_bind,
                    ..
                } => {
                    ensure!(
                        desc_bind.set() == descriptor_set_index,
                        format!(
                            "Descriptor set index mismatch, expected {}, got {}",
                            descriptor_set_index,
                            desc_bind.set()
                        )
                    );
                    match desc_ty {
                        DescriptorType::CombinedImageSampler() => {
                            samplers.push(NamedResourceBinding {
                                name: name.clone().with_context(|| format!("Failed to get name for combined image sampler at binding={}", desc_bind.bind()))?,
                                binding: desc_bind.bind(),
                            });
                        }
                        DescriptorType::StorageBuffer(_) => {
                            buffers.push(NamedResourceBinding {
                                name: name.clone().with_context(|| {
                                    format!(
                                        "Failed to get name for storage buffer at binding={}",
                                        desc_bind.bind()
                                    )
                                })?,
                                binding: desc_bind.bind(),
                            });
                        }
                        DescriptorType::UniformBuffer() => match ty {
                            Type::Struct(struct_type) => {
                                for member in &struct_type.members {
                                    let name = member
                                        .name
                                        .clone()
                                        .context("Failed to get name for uniform variable")?;
                                    if name.starts_with(GLOBAL_UNIFORM_PREFIX) {
                                        let global_type = GlobalType::from_str(
                                            &name[(GLOBAL_UNIFORM_PREFIX.len())..],
                                        )
                                        .with_context(|| {
                                            format!("Unknown global type: {:?}", name)
                                        })?;
                                        global_uniform_bindings.push(GlobalUniformMapping {
                                            global_type,
                                            f32_offset: member.offset as usize / size_of::<f32>(),
                                        });
                                    } else {
                                        let f32_count = match &member.ty {
                                            Type::Scalar(Float(_)) => Some(1),
                                            Type::Vector(vtype) => match vtype.scalar_ty {
                                                Float(_) => Some(vtype.nscalar),
                                                _ => None,
                                            },
                                            _ => None,
                                        };
                                        let Some(f32_count) = f32_count else {
                                            bail!("Uniform variable {name} is not a float scalar or float vector");
                                        };
                                        uniform_buffer_size = std::cmp::max(
                                            uniform_buffer_size,
                                            member.offset + f32_count as usize * size_of::<f32>(),
                                        );
                                        local_uniform_bindings.push(
                                            ShaderCompilationLocalUniform {
                                                name,
                                                f32_offset: member.offset / size_of::<f32>(),
                                                f32_count: f32_count as usize,
                                            },
                                        );
                                    }
                                }
                            }
                            _ => {
                                bail!("Unsupported uniform buffer type {:?}", desc_ty);
                            }
                        },
                        _ => {
                            bail!("Unsupported descriptor type {:?}", desc_ty);
                        }
                    }
                }
                Variable::Input { .. } => {}
                Variable::Output { .. } => {}
                Variable::PushConstant { .. } => {}
                Variable::SpecConstant { .. } => {}
            }
        }

        debug!(
            "Found {} samplers and {} buffers, SPIRV size: {}.",
            samplers.len(),
            buffers.len(),
            spirv_binary.len()
        );

        let result = ShaderArtifact {
            module,
            samplers,
            buffers,
            local_uniform_bindings,
            global_uniform_bindings,
            uniform_buffer_size,
        };

        trace!(
            "Local uniforms: {:?}",
            result
                .local_uniform_bindings
                .iter()
                .map(|u| &u.name)
                .collect::<Vec<_>>()
        );
        trace!(
            "Global uniforms: {:?}",
            result
                .global_uniform_bindings
                .iter()
                .map(|u| u.global_type)
                .collect::<Vec<_>>()
        );
        trace!(
            "Textures: {:?}",
            result.samplers.iter().map(|u| &u.name).collect::<Vec<_>>()
        );
        Ok(result)
    }

    fn from_spirv_binary_2(
        context: &Arc<VulkanContext>,
        kind: shaderc::ShaderKind,
        spirv_binary: &[u8],
    ) -> Result<Self> {
        // Extract metadata from SPIRV
        println!("ASFDFF");
        thread::sleep(std::time::Duration::from_millis(1000));

        let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary)
            .map_err(|err| anyhow!("Failed to reflect SPIRV binary: {err}"))?;
        println!("ASFDFF 2");
        let entry_point = reflect
            .enumerate_entry_points()
            .map_err(|err| anyhow!("Failed to enumerate entry points: {err}"))?
            .into_iter()
            .find(|ep| ep.name == "main")
            .context("Failed to find entry point 'main'")?;
        println!("ASFDFF 3");

        let module = unsafe { ShaderModule::from_bytes(context.device.clone(), spirv_binary) }?;

        let descriptor_set_index = match kind {
            shaderc::ShaderKind::Vertex => 0,
            shaderc::ShaderKind::Fragment => 1,
            shaderc::ShaderKind::Compute => 0,
            _ => panic!("Unsupported shader kind"),
        };

        // Find the descriptor set that belongs to the current shader stage
        let Some(descriptor_set) = entry_point
            .descriptor_sets
            .iter()
            .find(|ds| ds.set == descriptor_set_index)
        else {
            // The entire descriptor set is empty, so we can just use the module
            return Ok(ShaderArtifact {
                module,
                samplers: vec![],
                buffers: vec![],
                local_uniform_bindings: vec![],
                global_uniform_bindings: vec![],
                uniform_buffer_size: 0,
            });
        };

        // Find all samplers
        let samplers: Vec<_> = descriptor_set
            .bindings
            .iter()
            .filter(|binding| {
                binding.descriptor_type == ReflectDescriptorType::CombinedImageSampler
            })
            .map(|binding| NamedResourceBinding {
                name: binding.name.clone(),
                binding: binding.binding,
            })
            .collect();

        // Find all buffers
        let buffers: Vec<_> = descriptor_set
            .bindings
            .iter()
            .filter(|binding| binding.descriptor_type == ReflectDescriptorType::StorageBuffer)
            .map(|binding| NamedResourceBinding {
                name: binding.name.clone(),
                binding: binding.binding,
            })
            .collect();
        debug!(
            "Found {} samplers and {} buffers, SPIRV size: {}.",
            samplers.len(),
            buffers.len(),
            spirv_binary.len()
        );

        // Find the uniform block that contains all local and global uniforms
        let uniform_block = &descriptor_set
            .bindings
            .iter()
            .find(|binding| binding.descriptor_type == ReflectDescriptorType::UniformBuffer);

        // Find local and global uniforms
        let (local_uniform_bindings, global_uniform_bindings, uniform_buffer_size) =
            match uniform_block {
                Some(binding) => {
                    let members = &binding.block.members;
                    let local_uniform_bindings = members
                        .iter()
                        .filter(|var| !var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
                        .map(|var| {
                            let Some(type_desc) = &var.type_description else {
                                bail!(
                                    "Failed to get type description for uniform variable {}",
                                    var.name
                                );
                            };
                            ensure!(
                                (type_desc.type_flags & !ReflectTypeFlags::VECTOR)
                                    == ReflectTypeFlags::FLOAT,
                                "Uniform variable {} is not a float or vector",
                                var.name
                            );
                            Ok(ShaderCompilationLocalUniform {
                                name: var.name.clone(),
                                f32_offset: var.offset as usize / size_of::<f32>(),
                                f32_count: var.size as usize / size_of::<f32>(),
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let global_uniform_bindings = members
                        .iter()
                        .filter(|var| var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
                        .map(|var| {
                            GlobalType::from_str(&var.name[(GLOBAL_UNIFORM_PREFIX.len())..]).map(
                                |global_type| GlobalUniformMapping {
                                    global_type,
                                    f32_offset: var.offset as usize / size_of::<f32>(),
                                },
                            )
                        })
                        .collect::<::core::result::Result<Vec<_>, _>>()?;
                    let uniform_buffer_size = binding.block.size as usize;
                    (
                        local_uniform_bindings,
                        global_uniform_bindings,
                        uniform_buffer_size,
                    )
                }
                None => {
                    warn!("Shader has no uniform block.");
                    (vec![], vec![], 0)
                }
            };

        let result = ShaderArtifact {
            module,
            samplers,
            buffers,
            local_uniform_bindings,
            global_uniform_bindings,
            uniform_buffer_size,
        };

        trace!(
            "Local uniforms: {:?}",
            result
                .local_uniform_bindings
                .iter()
                .map(|u| &u.name)
                .collect::<Vec<_>>()
        );
        trace!(
            "Global uniforms: {:?}",
            result
                .global_uniform_bindings
                .iter()
                .map(|u| u.global_type)
                .collect::<Vec<_>>()
        );
        trace!(
            "Textures: {:?}",
            result.samplers.iter().map(|u| &u.name).collect::<Vec<_>>()
        );
        Ok(result)
    }
}
