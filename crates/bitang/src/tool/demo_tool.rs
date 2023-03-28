use crate::control::controls::Controls;
use crate::file::resource_repository::ResourceRepository;
use crate::render::chart::Chart;
use crate::render::material::MaterialStepType;
use crate::render::render_target::{Pass, RenderTarget, RenderTargetRole};
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::{RenderContext, VulkanApp, VulkanContext};
use crate::render::RenderObject;
use crate::tool::ui::Ui;
use anyhow::Result;
use glam::{Mat4, Vec3};
use std::cmp::max;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo};
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::{DeviceImageView, SwapchainImageView, VulkanoWindowRenderer};
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

pub struct DemoTool {
    // render_target: Arc<Pass>,
    ui: Ui,
    start_time: Instant,
    resource_repository: ResourceRepository,
    // render_unit: Option<RenderUnit>,
    // render_object: Option<Arc<RenderObject>>,
    chart: Arc<Chart>,
    // pass: Pass,
    controls: Controls,
    time: f32,
}

impl DemoTool {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<DemoTool> {
        let mut resource_repository = ResourceRepository::try_new()?;
        let mut controls = Controls::new();

        // let render_target = Arc::new(Pass::new(&context));
        // let render_target =
        //     RenderTarget::from_swapchain(RenderTargetRole::Color, context.swapchain_format)?;
        let chart = resource_repository.load_root_document(context, &mut controls)?;
        // let render_unit = RenderUnit::new(context, &render_target, render_object.clone());
        // let pass = Pass::new(context, &render_object, MaterialStepType::Opaque);

        let ui = Ui::new(context, event_loop);

        let demo_tool = DemoTool {
            // render_target,
            ui,
            start_time: Instant::now(),
            resource_repository,
            chart,
            controls,
            time: 5.0,
        };
        Ok(demo_tool)
    }

    // fn update_render_unit(&mut self, context: &VulkanContext) {
    //     let render_object = self
    //         .resource_repository
    //         .load_root_document(context, &mut self.controls);
    //     if let Ok(render_object) = render_object {
    //         if let Some(old_object) = &self.render_object {
    //             if Arc::ptr_eq(&render_object, old_object) {
    //                 return;
    //             }
    //         }
    //         self.render_object = Some(render_object.clone());
    //         self.render_unit = Some(RenderUnit::new(context, &self.render_target, render_object));
    //     }
    // }

    pub fn draw(
        &mut self,
        context: &mut RenderContext,
        before_future: Box<dyn GpuFuture>,
    ) -> Box<dyn GpuFuture> {
        // self.update_render_unit(context);

        let elapsed = self.start_time.elapsed().as_secs_f32();

        // Evaluate control splines
        for control in &mut self.controls.used_controls {
            control.evaluate_splines(self.time);
        }

        // let dimensions = target_image.dimensions().width_height();
        // let framebuffer = Framebuffer::new(
        //     self.render_target.vulkan_render_pass.clone(),
        //     FramebufferCreateInfo {
        //         attachments: vec![target_image, depth_image],
        //         ..Default::default()
        //     },
        // )
        // .unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            &context.command_buffer_allocator,
            context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // let clear_values = vec![Some([0.03, 0.03, 0.03, 1.0].into()), Some(1f32.into())];
        // builder
        //     .begin_render_pass(
        //         RenderPassBeginInfo {
        //             clear_values,
        //             ..RenderPassBeginInfo::framebuffer(framebuffer)
        //         },
        //         SubpassContents::Inline,
        //     )
        //     .unwrap()
        //     .set_viewport(0, [viewport.clone()]);

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

        self.chart.render(context);

        // builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let after_future = before_future
            .then_execute(context.context.graphics_queue().clone(), command_buffer)
            .unwrap()
            .boxed();

        after_future
    }
}

impl VulkanApp for DemoTool {
    fn paint(&mut self, context: &mut RenderContext, renderer: &mut VulkanoWindowRenderer) {
        let before_future = renderer.acquire().unwrap();
        // let target_image = renderer.swapchain_image_view();
        // let depth_image = renderer.get_additional_image_view(1);
        let scale_factor = renderer.window().scale_factor() as f32;

        let size = context.screen_buffer.dimensions();
        let movie_height = (size.width() * 9 / 16) as i32;
        let ui_height = max(size.height() as i32 - movie_height, 0) as f32 / scale_factor;

        let render_viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [size.width() as f32, movie_height as f32],
            depth_range: 0.0..1.0,
        };

        // Render app
        let app_finished_future = self.draw(context, before_future);

        // Render UI
        let gui_finished_future = self.ui.render(
            context,
            app_finished_future,
            ui_height,
            &mut self.controls,
            &mut self.time,
        );

        renderer.present(gui_finished_future, true);
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        self.ui.handle_window_event(event);
    }
}
