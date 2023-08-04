// use crate::render::vulkan_window::RenderContext;
// use anyhow::Result;
// use std::cell::RefCell;
// use std::sync::Arc;
// use vulkano::format::Format;
// use vulkano::image::view::{ImageView, ImageViewCreateInfo};
// use vulkano::image::{AttachmentImage, ImageUsage, ImageViewAbstract};
// use vulkano::memory::allocator::StandardMemoryAllocator;
// 
// #[derive(PartialEq, Eq)]
// pub enum RenderTargetRole {
//     Color,
//     Depth,
// }
// 
// #[derive(Clone)]
// pub enum RenderTargetSizeConstraint {
//     Static { width: u32, height: u32 },
//     ScreenRelative { width: f32, height: f32 },
// }
// 
// pub struct RenderTargetImage {
//     pub image_view: Arc<dyn ImageViewAbstract>,
//     pub texture: Option<Arc<AttachmentImage>>,
//     pub texture_size: (u32, u32),
// }
// 
// pub struct Image {
//     pub is_swapchain: bool,
//     pub id: String,
//     pub format: Format,
//     pub size_constraint: RenderTargetSizeConstraint,
//     pub role: RenderTargetRole,
//     pub image: RefCell<Option<RenderTargetImage>>,
// }
// 
// impl Image {
//     //! Swapchain render targets acquire their image view later before rendering
//     pub fn from_swapchain(role: RenderTargetRole, format: Format) -> Arc<Image> {
//         let id = match role {
//             RenderTargetRole::Color => "screen",
//             RenderTargetRole::Depth => "screen_depth",
//         };
//         Arc::new(Image {
//             is_swapchain: true,
//             id: id.to_string(),
//             format,
//             size_constraint: RenderTargetSizeConstraint::ScreenRelative {
//                 width: 1.0,
//                 height: 1.0,
//             },
//             role,
//             image: RefCell::new(None),
//         })
//     }
// 
//     pub fn update_swapchain_image(&self, image_view: Arc<dyn ImageViewAbstract>) {
//         // TODO: check if format is the same
//         *self.image.borrow_mut() = Some(RenderTargetImage {
//             texture: None,
//             texture_size: (
//                 image_view.dimensions().width(),
//                 image_view.dimensions().height(),
//             ),
//             image_view,
//         });
//     }
// 
//     pub fn new(
//         id: &str,
//         role: RenderTargetRole,
//         size_constraint: RenderTargetSizeConstraint,
//     ) -> Arc<Image> {
//         let format = match role {
//             RenderTargetRole::Color => Format::R16G16B16A16_SFLOAT,
//             RenderTargetRole::Depth => Format::D32_SFLOAT,
//         };
//         Arc::new(Image {
//             is_swapchain: false,
//             id: id.to_string(),
//             format,
//             size_constraint,
//             role,
//             image: RefCell::new(None),
//         })
//     }
// 
//     pub fn new_fake_swapchain(
//         memory_allocator: &Arc<StandardMemoryAllocator>,
//         role: RenderTargetRole,
//         texture_size: (u32, u32),
//     ) -> Arc<Image> {
//         let id = match role {
//             RenderTargetRole::Color => "screen",
//             RenderTargetRole::Depth => "screen_depth",
//         };
//         let format = match role {
//             RenderTargetRole::Color => Format::R8G8B8A8_SRGB,
//             RenderTargetRole::Depth => Format::D32_SFLOAT,
//         };
//         let mut usage = ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC;
//         if role == RenderTargetRole::Color {
//             usage |= ImageUsage::COLOR_ATTACHMENT;
//         } else if role == RenderTargetRole::Depth {
//             usage |= ImageUsage::DEPTH_STENCIL_ATTACHMENT;
//         }
//         let texture = AttachmentImage::with_usage(
//             memory_allocator,
//             [texture_size.0, texture_size.1],
//             format,
//             usage,
//         )
//         .unwrap();
//         let create_info = ImageViewCreateInfo {
//             usage,
//             ..ImageViewCreateInfo::from_image(&texture)
//         };
//         let image = Some(RenderTargetImage {
//             image_view: ImageView::new(texture.clone(), create_info).unwrap(),
//             texture: Some(texture),
//             texture_size,
//         });
// 
//         Arc::new(Image {
//             is_swapchain: true,
//             id: id.to_string(),
//             format,
//             size_constraint: RenderTargetSizeConstraint::Static {
//                 width: texture_size.0,
//                 height: texture_size.1,
//             },
//             role,
//             image: RefCell::new(image),
//         })
//     }
// 
//     pub fn ensure_buffer(&self, context: &RenderContext) -> Result<()> {
//         if self.is_swapchain {
//             return Ok(());
//         }
//         let texture_size = match self.size_constraint {
//             RenderTargetSizeConstraint::Static { width, height } => (width, height),
//             RenderTargetSizeConstraint::ScreenRelative { width, height } => {
//                 let dimensions = &context.screen_viewport.dimensions;
//                 (
//                     (dimensions[0] * width) as u32,
//                     (dimensions[1] * height) as u32,
//                 )
//             }
//         };
// 
//         // Skip if texture size is the same
//         if let Some(image) = self.image.borrow().as_ref() {
//             if image.texture_size == texture_size {
//                 return Ok(());
//             }
//         }
// 
//         let texture = AttachmentImage::with_usage(
//             context.vulkan_context.context.memory_allocator(),
//             [texture_size.0, texture_size.1],
//             self.format,
//             ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
//         )?;
//         *self.image.borrow_mut() = Some(RenderTargetImage {
//             image_view: ImageView::new_default(texture.clone())?,
//             texture: Some(texture),
//             texture_size,
//         });
//         Ok(())
//     }
// }
