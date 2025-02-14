use crate::control::controls::GlobalType;
use crate::loader::file_cache::{ContentHash, FileCache};
use crate::loader::resource_path::ResourcePath;
use crate::render::shader::{GlobalUniformMapping, ShaderKind};
use crate::tool::WindowContext;
use ahash::AHashSet;
use anyhow::{bail, ensure, Context, Result};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use itertools::Itertools;
use log::{error, warn};
use naga::front::wgsl::ParseError;
use naga::valid::Capabilities;
use naga::{Module, ShaderStage};
use spirq::ty::ScalarType::Float;
use spirq::ty::{DescriptorType, SpirvType, Type, VectorType};
use spirq::var::Variable;
use spirq::ReflectConfig;
use std::cell::RefCell;
use std::error::Error;
use std::mem::size_of;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use tokio::task;
use tracing::{debug, info, instrument, trace};
use vulkano::shader;
use vulkano::shader::{ShaderModule, ShaderModuleCreateInfo};

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
    #[instrument(skip(context, kind, file_hash_cache))]
    pub fn compile_shader(
        context: &Arc<WindowContext>,
        path: &ResourcePath,
        kind: ShaderKind,
        file_hash_cache: Arc<FileCache>,
        macros: Vec<(String, String)>,
    ) -> Result<Self> {
        let now = std::time::Instant::now();

        let source_file = {
            let file_hash_cache = Arc::clone(&file_hash_cache);
            tokio::runtime::Handle::current()
                .block_on(async move { file_hash_cache.get(path).await })
        }?;
        let source = std::str::from_utf8(&source_file.content)
            .with_context(|| format!("Shader source file is not UTF-8: '{:?}'", path))?;

        let spirv = {
            // TODO: report code spans on the top level, not here
            let mut frontend = naga::front::wgsl::Frontend::new();
            let res = match frontend.parse(source) {
                Ok(res) => res,
                Err(err) => {
                    let mut files = SimpleFiles::new();
                    let file_id = files.add(path.to_pwd_relative_path().unwrap(), source);

                    let labels = err
                        .labels()
                        .map(|(span, msg)| {
                            Label::primary(file_id, span.to_range().unwrap()).with_message(msg)
                        })
                        .collect_vec();
                    let diagnostic = Diagnostic::error()
                        .with_labels(labels)
                        .with_message(err.message());

                    let writer = StandardStream::stderr(ColorChoice::Always);
                    let config = codespan_reporting::term::Config::default();

                    codespan_reporting::term::emit(
                        &mut writer.lock(),
                        &config,
                        &files,
                        &diagnostic,
                    )?;

                    bail!(
                        "Failed to parse shader source file '{path:?}', error: {}",
                        err.message()
                    );
                }
            };

            let res_clone = res.clone();
            let module_info = thread::spawn(move || {
                let mut validator = naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    Capabilities::all(),
                );
                validator.validate(&res_clone)
            }).join()
                .map_err(|err| anyhow::anyhow!("Failed to validate shader source file '{:?}': {:?}", path, err))?;
            let module_info = match module_info {
                Ok(res) => res,
                Err(err) => {
                    let mut files = SimpleFiles::new();
                    let file_id = files.add(path.to_pwd_relative_path().unwrap(), source);

                    let labels = err
                        .spans()
                        .map(|(span, msg)| {
                            Label::primary(file_id, span.to_range().unwrap()).with_message(msg)
                        })
                        .collect_vec();
                    let diagnostic = Diagnostic::error()
                        .with_labels(labels)
                        .with_message(format!("{:?}", err.source()));
                    let writer = StandardStream::stderr(ColorChoice::Always);
                    let config = codespan_reporting::term::Config::default();

                    codespan_reporting::term::emit(
                        &mut writer.lock(),
                        &config,
                        &files,
                        &diagnostic,
                    )?;

                    bail!(
                       "Failed to parse shader source file '{path:?}', error: {err:?}",
                        );
                }
            };

            let mut spv_options = naga::back::spv::Options::default();
            spv_options.flags =
                naga::back::spv::WriterFlags::DEBUG | naga::back::spv::WriterFlags::LABEL_VARYINGS;
            let spirv_u32 = naga::back::spv::write_vec(&res, &module_info, &spv_options, None)?;
            let spirv_u8 = spirv_u32
                .iter()
                .flat_map(|&w| w.to_le_bytes().to_vec())
                .collect::<Vec<u8>>();
            spirv_u8
        };
        info!("compiled in {:?}.", now.elapsed());

        let shader_artifact = ShaderArtifact::from_spirv_binary(context, kind, &spirv)?;

        Ok(Self {
            shader_artifact,
            include_chain: vec![],
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
    pub textures: Vec<NamedResourceBinding>,
    pub buffers: Vec<NamedResourceBinding>,
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,
    pub local_uniform_bindings: Vec<ShaderCompilationLocalUniform>,

    /// The size of the uniform buffer in 32-bit floats
    pub uniform_buffer_size: usize,
}

impl ShaderArtifact {
    fn from_spirv_binary(
        context: &Arc<WindowContext>,
        kind: ShaderKind,
        spirv_binary: &[u8],
    ) -> Result<Self> {
        // Extract metadata from SPIRV
        let entry_points = ReflectConfig::new()
            .spv(spirv_binary)
            .ref_all_rscs(true)
            .combine_img_samplers(true)
            .gen_unique_names(false)
            .reflect()?;
        let main_function = match kind {
            ShaderKind::Vertex => "vs_main",
            ShaderKind::Fragment => "fs_main",
            ShaderKind::Compute => "main",
        };
        let entry_point = entry_points
            .iter()
            .find(|ep| ep.name == main_function)
            .context("Failed to find entry point 'main'")?;

        let module = unsafe {
            let shader_words = shader::spirv::bytes_to_words(spirv_binary)?;
            ShaderModule::new(
                context.device.clone(),
                ShaderModuleCreateInfo::new(&shader_words),
            )
        }?;

        let descriptor_set_index = match kind {
            ShaderKind::Vertex => 0,
            ShaderKind::Fragment => 1,
            ShaderKind::Compute => 0,
        };

        // Collect the actually used bindings. Spirq doesn't always get us the same results
        // as Vulkano's pipeline layout, so we need to filter out the unused bindings.
        let used_bindings = &module
            .entry_point(main_function)
            .context("Failed to get entry point")?
            .info()
            .descriptor_binding_requirements
            .keys()
            .map(|(_set, binding)| *binding)
            .collect::<AHashSet<_>>();

        let mut samplers = Vec::new();
        let mut textures = Vec::new();
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
                    if desc_bind.set() != descriptor_set_index {
                        continue;
                    }
                    ensure!(
                        desc_bind.set() == descriptor_set_index,
                        format!(
                            "Descriptor set index mismatch, expected {}, got {}",
                            descriptor_set_index,
                            desc_bind.set()
                        )
                    );
                    let binding = desc_bind.bind();
                    if !used_bindings.contains(&binding) {
                        debug!("Skipping binding {} not in requirements", binding);
                        continue;
                    }
                    match desc_ty {
                        DescriptorType::Sampler() => {
                            samplers.push(NamedResourceBinding {
                                name: name.clone().with_context(|| {
                                    format!("Failed to get name for sampler at binding={binding}")
                                })?,
                                binding,
                            });
                        }
                        DescriptorType::SampledImage() => {
                            textures.push(NamedResourceBinding {
                                name: name.clone().with_context(|| {
                                    format!("Failed to get name for texture at binding={binding}")
                                })?,
                                binding,
                            });
                        }
                        DescriptorType::StorageBuffer(_) => {
                            buffers.push(NamedResourceBinding {
                                name: name.clone().with_context(|| format!("Failed to get name for storage buffer at binding={binding}"))?,
                                binding,
                            });
                        }
                        DescriptorType::UniformBuffer() => match ty {
                            Type::Struct(struct_type) => {
                                for member in &struct_type.members {
                                    // warn!("MEMBER: {:#?}", member);
                                    match &member.ty {
                                        Type::Struct(struct_type) => {
                                            for member in &struct_type.members {
                                                let name = member.name.clone().context(
                                                    "Failed to get name of uniform variable",
                                                )?;
                                                let f32_offset = member.offset.with_context(|| {
                                                    format!("Failed to get offset for uniform variable {name}")
                                                })? / size_of::<f32>();

                                                if let Some(global_name) =
                                                    name.strip_prefix(GLOBAL_UNIFORM_PREFIX)
                                                {
                                                    let global_type =
                                                        GlobalType::from_str(global_name)
                                                            .with_context(|| {
                                                                format!(
                                                                    "Unknown global: {:?}",
                                                                    name
                                                                )
                                                            })?;
                                                    global_uniform_bindings.push(
                                                        GlobalUniformMapping {
                                                            global_type,
                                                            f32_offset,
                                                        },
                                                    );
                                                } else {
                                                    let f32_count = match &member.ty {
                                                        Type::Scalar(Float { bits: 32 }) => 1,
                                                        Type::Vector(VectorType { scalar_ty: Float { bits: 32 }, nscalar, }) => *nscalar,
                                                        _ => bail!("Uniform variable {name} is not a float scalar or float vector"),
                                                    };
                                                    local_uniform_bindings.push(
                                                        ShaderCompilationLocalUniform {
                                                            name,
                                                            f32_offset,
                                                            f32_count: f32_count as usize,
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                        _ => {
                                            error!(
                                                "Unsupported uniform buffer member type {:?}",
                                                member.ty
                                            );
                                        }
                                    }
                                }
                                let byte_size = struct_type.nbyte().with_context(|| {
                                    format!("Failed to get byte size of uniform struct {name:?}")
                                })?;
                                uniform_buffer_size = byte_size / size_of::<f32>();
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
            textures,
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
