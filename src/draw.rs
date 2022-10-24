use crate::render::VulkanRenderer;
use bytemuck::{Pod, Zeroable};
use std::cmp::max;
use std::sync::Arc;
use std::time::Instant;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SubpassContents};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageAccess, ImageUsage, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::swapchain::{AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture};
use vulkano::{swapchain, sync};
use vulkano_win::VkSurfaceBuild;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

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

pub struct VulkanRenderer {
    current_frame: usize,
    framebuffers: Vec<Arc<Framebuffer>>,
    swapchain: Arc<Swapchain<Window>>,
    surface: Arc<swapchain::Surface<Window>>,
    render_pass: Arc<RenderPass>,
}

pub struct VulkanApp {
    renderer: VulkanRenderer,
    event_loop: EventLoop<()>,
    viewport: Viewport,
}

impl VulkanApp {
    pub fn new() -> Self {
        let required_extensions = vulkano_win::required_extensions();
        let instance = Instance::new(InstanceCreateInfo {
            enabled_extensions: required_extensions,
            ..Default::default()
        })
        .unwrap();

        let physical = PhysicalDevice::enumerate(&instance).next().unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical.properties().device_name,
            physical.properties().device_type,
        );

        let event_loop = EventLoop::new();
        let surface = WindowBuilder::new()
            .with_title("egui_vulkano demo")
            // .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .build_vk_surface(&event_loop, instance.clone())
            .unwrap();

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };

        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter(|&p| p.supported_extensions().is_superset_of(&device_extensions))
            .filter_map(|p| {
                p.queue_families()
                    .find(|&q| {
                        q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false)
                    })
                    .map(|q| (p, q))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
            })
            .unwrap();

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

        // let mut renderer = VulkanRenderer::new();
        // renderer.run();

        let (swapchain, images) = {
            let caps = physical_device
                .surface_capabilities(&surface, Default::default())
                .unwrap();
            let composite_alpha = caps.supported_composite_alpha.iter().next().unwrap();

            let image_format = Some(Format::B8G8R8A8_SRGB);
            let image_extent: [u32; 2] = surface.window().inner_size().into();

            Swapchain::new(
                device.clone(),
                surface.clone(),
                SwapchainCreateInfo {
                    min_image_count: caps.min_image_count,
                    image_format,
                    image_extent,
                    image_usage: ImageUsage::color_attachment(),
                    composite_alpha,

                    ..Default::default()
                },
            )
            .unwrap()
        };

        let render_pass = vulkano::ordered_passes_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                }
            },
            passes: [
                { color: [color], depth_stencil: {}, input: [] }
                // { color: [color], depth_stencil: {}, input: [] } // Create a second renderpass to draw egui
            ]
        )
        .unwrap();

        let framebuffers = window_size_dependent_setup(&images, render_pass.clone(), &mut viewport);

        let mut renderer = VulkanRenderer {
            current_frame: 0,
            framebuffers,
            swapchain,
            surface,
            render_pass,
        };

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        VulkanApp {
            renderer,
            event_loop,
            viewport,
        }
    }

    pub fn main_loop(mut self) {
        let mut recreate_swapchain = false;
        let mut previous_frame_end = Some(FrameEndFuture::now(self.device.clone()));
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
                Event::WindowEvent { event: _, .. } => {
                    // let egui_consumed_event = egui_winit.on_event(&egui_ctx, &event);
                    // if !egui_consumed_event {
                    //     // do your own event handling here
                    // };
                }
                Event::RedrawEventsCleared => {
                    previous_frame_end
                        .as_mut()
                        .unwrap()
                        .as_mut()
                        .cleanup_finished();

                    if recreate_swapchain {
                        let dimensions: [u32; 2] = self.surface.window().inner_size().into();
                        let (new_swapchain, new_images) =
                            match self.swapchain.recreate(SwapchainCreateInfo {
                                image_extent: self.surface.window().inner_size().into(),
                                ..self.swapchain.create_info()
                            }) {
                                Ok(r) => r,
                                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => {
                                    return
                                }
                                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                            };

                        self.swapchain = new_swapchain;
                        self.framebuffers = window_size_dependent_setup(
                            &new_images,
                            self.render_pass.clone(),
                            &mut self.viewport,
                        );
                        self.viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];
                        recreate_swapchain = false;
                    }

                    let (image_num, suboptimal, acquire_future) =
                        match swapchain::acquire_next_image(self.swapchain.clone(), None) {
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

                    let clear_values = vec![[0.0, 0.0, 1.0, 1.0].into()];
                    let mut builder = AutoCommandBufferBuilder::primary(
                        self.device.clone(),
                        self.queue.family(),
                        CommandBufferUsage::OneTimeSubmit,
                    )
                    .unwrap();

                    let _frame_start = Instant::now();
                    // egui_ctx.begin_frame(egui_winit.take_egui_input(surface.window()));
                    // demo_windows.ui(&egui_ctx);

                    // egui::Window::new("Color test")
                    //     .vscroll(true)
                    //     .show(&egui_ctx, |ui| {
                    //         egui_test.ui(ui);
                    //     });
                    //
                    // egui::Window::new("Settings").show(&egui_ctx, |ui| {
                    //     egui_ctx.settings_ui(ui);
                    // });
                    //
                    // egui::Window::new("Benchmark")
                    //     .default_height(600.0)
                    //     .show(&egui_ctx, |ui| {
                    //         egui_bench.draw(ui);
                    //     });
                    //
                    // egui::Window::new("Texture test").show(&egui_ctx, |ui| {
                    //     ui.image(my_texture.id(), (200.0, 200.0));
                    //     if ui.button("Reload texture").clicked() {
                    //         // previous TextureHandle is dropped, causing egui to free the texture:
                    //         my_texture = egui_ctx.load_texture("my_texture", ColorImage::example());
                    //     }
                    // });

                    let size = self.surface.window().inner_size();
                    let movie_height = (size.width * 9 / 16) as i32;
                    let _panel_height = max(size.height as i32 - movie_height, 0) as f32;

                    // egui::TopBottomPanel::bottom("my_panel")
                    //     .height_range(panel_height..=panel_height)
                    //     .show(&egui_ctx, |ui| {
                    //         ui.with_layout(
                    //             egui::Layout::top_down_justified(egui::Align::Center),
                    //             |ui| {
                    //                 ui.button("I am becoming wider as needed");
                    //                 ui.allocate_space(ui.available_size());
                    //             },
                    //         );
                    //         // ui.label("Hello World!");
                    //     });

                    // // Get the shapes from egui
                    // let egui_output = egui_ctx.end_frame();
                    // let platform_output = egui_output.platform_output;
                    // egui_winit.handle_platform_output(surface.window(), &egui_ctx, platform_output);

                    // let result = egui_painter
                    //     .update_textures(egui_output.textures_delta, &mut builder)
                    //     .expect("egui texture error");

                    let wait_for_last_frame = true; // result == UpdateTexturesResult::Changed;

                    // Do your usual rendering
                    builder
                        .begin_render_pass(
                            self.framebuffers[image_num].clone(),
                            SubpassContents::Inline,
                            clear_values,
                        )
                        .unwrap()
                        .set_viewport(0, [self.viewport.clone()])
                        .bind_pipeline_graphics(self.pipeline.clone())
                        .bind_vertex_buffers(0, self.vertex_buffer.clone())
                        .draw(self.vertex_buffer.len().try_into().unwrap(), 1, 0, 0)
                        .unwrap(); // Don't end the render pass yet

                    // Build your gui

                    // Automatically start the next render subpass and draw the gui
                    // let size = self.surface.window().inner_size();
                    // let sf: f32 = self.surface.window().scale_factor() as f32;
                    // egui_painter
                    //     .draw(
                    //         &mut builder,
                    //         [(size.width as f32) / sf, (size.height as f32) / sf],
                    //         &egui_ctx,
                    //         egui_output.shapes,
                    //     )
                    //     .unwrap();

                    // egui_bench.push(frame_start.elapsed().as_secs_f64());

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
                        .then_execute(self.queue.clone(), command_buffer)
                        .unwrap()
                        .then_swapchain_present(
                            self.queue.clone(),
                            self.swapchain.clone(),
                            image_num,
                        )
                        .then_signal_fence_and_flush();

                    match future {
                        Ok(future) => {
                            previous_frame_end = Some(FrameEndFuture::FenceSignalFuture(future));
                        }
                        Err(FlushError::OutOfDate) => {
                            recreate_swapchain = true;
                            previous_frame_end = Some(FrameEndFuture::now(self.device.clone()));
                        }
                        Err(e) => {
                            println!("Failed to flush future: {:?}", e);
                            previous_frame_end = Some(FrameEndFuture::now(self.device.clone()));
                        }
                    }
                }
                _ => (),
            }
        });
    }
}

fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
) -> Vec<Arc<Framebuffer>> {
    let dimensions = images[0].dimensions().width_height();
    viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}
