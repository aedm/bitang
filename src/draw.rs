use crate::render::VulkanRenderer;
use crate::DemoApp;
// use crate::Gui;
use bytemuck::{Pod, Zeroable};
use std::cmp::max;
use std::sync::Arc;
use std::time::Instant;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SubpassContents};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, DeviceOwned, Queue, QueueCreateInfo,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{AttachmentImage, ImageAccess, ImageUsage, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::swapchain::{AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture};
use vulkano::{swapchain, sync, VulkanLibrary};
use vulkano_win::VkSurfaceBuild;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::monitor::VideoMode;
use winit::window::{Fullscreen, Window, WindowBuilder};

pub const NUMBER_FRAMES_IN_FLIGHT: usize = 3;

pub enum FrameEndFuture<F: GpuFuture + 'static> {
    FenceSignalFuture(FenceSignalFuture<F>),
    BoxedFuture(Box<dyn GpuFuture>),
}

impl<F: GpuFuture> FrameEndFuture<F> {
    pub fn now(device: Arc<Device>) -> Self {
        Self::BoxedFuture(sync::now(device).boxed())
    }

    pub fn get(self) -> Box<dyn GpuFuture> {
        match self {
            FrameEndFuture::FenceSignalFuture(f) => f.boxed(),
            FrameEndFuture::BoxedFuture(f) => f,
        }
    }
}

impl<F: GpuFuture> AsMut<dyn GpuFuture> for FrameEndFuture<F> {
    fn as_mut(&mut self) -> &mut (dyn GpuFuture + 'static) {
        match self {
            FrameEndFuture::FenceSignalFuture(f) => f,
            FrameEndFuture::BoxedFuture(f) => f,
        }
    }
}

pub struct VulkanApp {
    pub renderer: VulkanRenderer,
    event_loop: EventLoop<()>,
    viewport: Viewport,
}

impl VulkanApp {
    pub fn new() -> Self {
        let library = VulkanLibrary::new().unwrap();
        let required_extensions = vulkano_win::required_extensions(&library);
        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                enabled_extensions: required_extensions,
                // Enable enumerating devices that use non-conformant vulkan implementations. (ex. MoltenVK)
                enumerate_portability: true,
                ..Default::default()
            },
        )
        .unwrap();

        let physical = PhysicalDevice::enumerate(&instance).next().unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical.properties().device_name,
            physical.properties().device_type,
        );

        let event_loop = EventLoop::new();
        let mode = event_loop
            .primary_monitor()
            .unwrap()
            .video_modes()
            .next()
            .unwrap();
        println!("Using mode: {:?}", mode);

        let surface = WindowBuilder::new()
            .with_title("bitang")
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0))
            // .with_fullscreen(Some(Fullscreen::Borderless(None)))
            // .with_fullscreen(Some(Fullscreen::Exclusive(mode)))
            .build_vk_surface(&event_loop, instance.clone())
            .unwrap();

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };

        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        // We select a queue family that supports graphics operations. When drawing to
                        // a window surface, as we do in this example, we also need to check that queues
                        // in this queue family are capable of presenting images to the surface.
                        q.queue_flags.graphics
                            && p.surface_support(i as u32, &surface).unwrap_or(false)
                    })
                    // The code here searches for the first queue family that is suitable. If none is
                    // found, `None` is returned to `filter_map`, which disqualifies this physical
                    // device.
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .expect("No suitable GPU found.");

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let (device, mut queues) = Device::new(
            physical_device,
            DeviceCreateInfo {
                enabled_extensions: physical_device
                    .required_extensions()
                    .union(&device_extensions),
                queue_create_infos: vec![QueueCreateInfo::family(queue_family)],
                ..Default::default()
            },
        )
        .unwrap();

        let queue = queues.next().unwrap();

        let (mut swapchain, images) = {
            // Querying the capabilities of the surface. When we create the swapchain we can only
            // pass values that are allowed by the capabilities.
            let surface_capabilities = device
                .physical_device()
                .surface_capabilities(&surface, Default::default())
                .unwrap();

            // Choosing the internal format that the images will have.
            let image_format = Some(
                device
                    .physical_device()
                    .surface_formats(&surface, Default::default())
                    .unwrap()[0]
                    .0,
            );
            let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();

            // Please take a look at the docs for the meaning of the parameters we didn't mention.
            Swapchain::new(
                device.clone(),
                surface.clone(),
                SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count,

                    image_format,
                    // The dimensions of the window, only used to initially setup the swapchain.
                    // NOTE:
                    // On some drivers the swapchain dimensions are specified by
                    // `surface_capabilities.current_extent` and the swapchain size must use these
                    // dimensions.
                    // These dimensions are always the same as the window dimensions.
                    //
                    // However, other drivers don't specify a value, i.e.
                    // `surface_capabilities.current_extent` is `None`. These drivers will allow
                    // anything, but the only sensible value is the window
                    // dimensions.
                    //
                    // Both of these cases need the swapchain to use the window dimensions, so we just
                    // use that.
                    image_extent: window.inner_size().into(),

                    image_usage: ImageUsage {
                        color_attachment: true,
                        ..ImageUsage::empty()
                    },

                    // The alpha mode indicates how the alpha value of the final image will behave. For
                    // example, you can choose whether the window will be opaque or transparent.
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .iter()
                        .next()
                        .unwrap(),

                    ..Default::default()
                },
            )
            .unwrap()
        };

        // TODO
        // let memory_allocator = StandardMemoryAllocator::new_default(device.clone());

        let render_pass = vulkano::ordered_passes_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16_UNORM,
                    samples: 1,
                }
            },
            passes: [
                { color: [color], depth_stencil: {depth}, input: [] },
                { color: [color], depth_stencil: {}, input: [] } // Create a second renderpass to draw egui
            ]
        )
        .unwrap();

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers =
            window_size_dependent_setup(&images, render_pass.clone(), &mut viewport, &device);

        let mut renderer = VulkanRenderer {
            device,
            queue,
            current_frame: 0,
            framebuffers,
            surface,
            render_pass,
            swapchain,
        };

        VulkanApp {
            renderer,
            event_loop,
            viewport,
        }
    }

    pub fn main_loop(mut self, mut app: DemoApp, mut gui: Gui) {
        let mut recreate_swapchain = false;
        let mut previous_frame_end = Some(FrameEndFuture::now(self.renderer.device.clone()));
        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    ..
                } => {
                    recreate_swapchain = true;
                }
                Event::WindowEvent { event, .. } => {
                    gui.handle_window_event(&event);
                }
                Event::RedrawEventsCleared => {
                    previous_frame_end
                        .as_mut()
                        .unwrap()
                        .as_mut()
                        .cleanup_finished();

                    if recreate_swapchain {
                        let dimensions: [u32; 2] =
                            self.renderer.surface.window().inner_size().into();
                        let (new_swapchain, new_images) =
                            match self.renderer.swapchain.recreate(SwapchainCreateInfo {
                                image_extent: self.renderer.surface.window().inner_size().into(),
                                ..self.renderer.swapchain.create_info()
                            }) {
                                Ok(r) => r,
                                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => {
                                    return
                                }
                                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                            };

                        self.renderer.swapchain = new_swapchain;
                        self.renderer.framebuffers = window_size_dependent_setup(
                            &new_images,
                            self.renderer.render_pass.clone(),
                            &mut self.viewport,
                            &self.renderer.device,
                        );
                        self.viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];
                        recreate_swapchain = false;
                    }

                    let (image_num, suboptimal, acquire_future) =
                        match swapchain::acquire_next_image(self.renderer.swapchain.clone(), None) {
                            Ok(r) => r,
                            Err(AcquireError::OutOfDate) => {
                                recreate_swapchain = true;
                                return;
                            }
                            Err(e) => panic!("Failed to acquire next image: {:?}", e),
                        };

                    if suboptimal {
                        recreate_swapchain = true;
                    }

                    let mut builder = AutoCommandBufferBuilder::primary(
                        self.renderer.device.clone(),
                        self.renderer.queue.family(),
                        CommandBufferUsage::OneTimeSubmit,
                    )
                    .unwrap();

                    let framebuffer = self.renderer.framebuffers[image_num].clone();

                    let sf = self.renderer.surface.window().scale_factor() as i32;

                    let size = self.renderer.surface.window().inner_size();
                    let movie_height = (size.width * 9 / 16) as i32;
                    let bottom_panel_height = max(size.height as i32 - movie_height, 0) / sf;

                    gui.build(&mut self.renderer, &mut builder, bottom_panel_height);

                    let render_viewport = Viewport {
                        origin: [0.0, 0.0],
                        dimensions: [size.width as f32, movie_height as f32],
                        depth_range: 0.0..1.0,
                    };
                    app.draw(
                        &mut self.renderer,
                        &mut builder,
                        framebuffer,
                        render_viewport,
                    );

                    let vp = self.viewport.clone();
                    // builder.set_viewport(0, [self.viewport.clone()]);
                    builder.set_viewport(0, [vp]);

                    gui.draw(&mut self.renderer, &mut builder);

                    let wait_for_last_frame = true; // result == UpdateTexturesResult::Changed;

                    // End the render pass as usual
                    builder.end_render_pass().unwrap();

                    let command_buffer = builder.build().unwrap();

                    if wait_for_last_frame {
                        if let Some(FrameEndFuture::FenceSignalFuture(ref mut f)) =
                            previous_frame_end
                        {
                            f.wait(None).unwrap();
                        }
                    }

                    let future = previous_frame_end
                        .take()
                        .unwrap()
                        .get()
                        .join(acquire_future)
                        .then_execute(self.renderer.queue.clone(), command_buffer)
                        .unwrap()
                        .then_swapchain_present(
                            self.renderer.queue.clone(),
                            self.renderer.swapchain.clone(),
                            image_num,
                        )
                        .then_signal_fence_and_flush();

                    match future {
                        Ok(future) => {
                            previous_frame_end = Some(FrameEndFuture::FenceSignalFuture(future));
                        }
                        Err(FlushError::OutOfDate) => {
                            recreate_swapchain = true;
                            previous_frame_end =
                                Some(FrameEndFuture::now(self.renderer.device.clone()));
                        }
                        Err(e) => {
                            println!("Failed to flush future: {:?}", e);
                            previous_frame_end =
                                Some(FrameEndFuture::now(self.renderer.device.clone()));
                        }
                    }
                }
                _ => (),
            }
        });
    }
}

fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
    device: &Arc<Device>,
) -> Vec<Arc<Framebuffer>> {
    let dimensions = images[0].dimensions().width_height();
    viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

    let depth_buffer = ImageView::new_default(
        AttachmentImage::transient(device.clone(), dimensions, Format::D16_UNORM).unwrap(),
    )
    .unwrap();

    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view, depth_buffer.clone()],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}
