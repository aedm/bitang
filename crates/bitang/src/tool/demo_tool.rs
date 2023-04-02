use crate::file::resource_repository::ResourceRepository;
use crate::render::chart::Chart;
use crate::render::vulkan_window::{RenderContext, VulkanApp, VulkanContext};
use crate::tool::ui::Ui;
use anyhow::Result;
use glam::{Mat4, Vec3};
use std::cmp::max;
use std::f32::consts::PI;
use std::time::Instant;
use tracing::error;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::VulkanoWindowRenderer;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

pub struct DemoTool {
    ui: Ui,
    start_time: Instant,
    resource_repository: ResourceRepository,
    time: f32,
}

impl DemoTool {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<DemoTool> {
        let mut resource_repository = ResourceRepository::try_new()?;
        let _ = resource_repository.load_root_document(context)?;
        let ui = Ui::new(context, event_loop);

        let demo_tool = DemoTool {
            ui,
            start_time: Instant::now(),
            resource_repository,
            time: 5.0,
        };
        Ok(demo_tool)
    }

    pub fn draw(&mut self, context: &mut RenderContext, chart: &Chart) {
        let elapsed = self.start_time.elapsed().as_secs_f32();

        // Evaluate control splines
        for control in &mut self.resource_repository.controls.used_controls {
            control.evaluate_splines(self.time);
        }

        let viewport = &context.screen_viewport;
        // We use a left-handed, y-up coordinate system.
        // Vulkan uses y-down, so we need to flip it back.
        let camera_from_world = Mat4::look_at_lh(
            Vec3::new(0.0, 0.0, -3.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
        );
        let world_from_model = Mat4::from_rotation_y(elapsed);

        // Vulkan uses a [0,1] depth range, ideal for infinite far plane
        let projection_from_camera = Mat4::perspective_infinite_lh(
            PI / 2.0,
            viewport.dimensions[0] / viewport.dimensions[1],
            0.1,
        );
        let projection_from_model = projection_from_camera * camera_from_world * world_from_model;
        let camera_from_model = camera_from_world * world_from_model;

        context.globals.projection_from_model = projection_from_model;
        context.globals.camera_from_model = camera_from_model;

        chart.render(context);
    }
}

impl VulkanApp for DemoTool {
    fn paint(&mut self, vulkan_context: &VulkanContext, renderer: &mut VulkanoWindowRenderer) {
        let Ok(chart) = self
            .resource_repository
            .load_root_document(vulkan_context) else {
            error!("Failed to load root document");
            return;
        };

        let before_future = renderer.acquire().unwrap();
        let target_image = renderer.swapchain_image_view();
        let depth_image = renderer.get_additional_image_view(1);

        let scale_factor = renderer.window().scale_factor() as f32;

        let size = target_image.dimensions();
        let movie_height = (size.width() * 9 / 16) as i32;
        let ui_height = max(size.height() as i32 - movie_height, 0) as f32 / scale_factor;

        let screen_viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [size.width() as f32, movie_height as f32],
            depth_range: 0.0..1.0,
        };

        let mut command_builder = AutoCommandBufferBuilder::primary(
            &vulkan_context.command_buffer_allocator,
            vulkan_context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        vulkan_context
            .swapchain_render_targets_by_id
            .get("screen")
            .unwrap()
            .update_swapchain_image(target_image.clone());
        vulkan_context
            .swapchain_render_targets_by_id
            .get("screen_depth")
            .unwrap()
            .update_swapchain_image(depth_image.clone());

        {
            let mut context = RenderContext {
                vulkan_context,
                screen_buffer: target_image,
                screen_viewport,
                command_builder: &mut command_builder,
                depth_buffer: depth_image,
                globals: Default::default(),
            };

            // Render app
            self.draw(&mut context, &chart);

            // Render UI
            self.ui.draw(
                &mut context,
                ui_height,
                &mut self.resource_repository.controls,
                &chart,
                &mut self.time,
            );
        }

        let command_buffer = command_builder.build().unwrap();

        let after_future = before_future
            .then_execute(
                vulkan_context.context.graphics_queue().clone(),
                command_buffer,
            )
            .unwrap()
            .boxed();

        renderer.present(after_future, true);
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        self.ui.handle_window_event(event);
    }
}
