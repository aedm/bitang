use crate::control::controls::ControlSet;
use crate::file::resource_repository::ResourceRepository;
use crate::render::chart::Chart;
use crate::render::project::Project;
use crate::render::vulkan_window::{RenderContext, VulkanApp, VulkanContext};
use crate::tool::ui::Ui;
use anyhow::Result;
use glam::{Mat4, Vec3};
use std::cmp::max;
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    has_render_failure: bool,
    ui_state: UiState,
    play_start_time: Instant,
    last_eval_time: f32,
}

pub struct UiState {
    pub project: Option<Arc<Project>>,
    pub selected_chart_id: Option<String>,
    pub time: f32,
    pub is_playing: bool,
}

impl UiState {
    pub fn get_chart(&self) -> Option<Arc<Chart>> {
        self.project.as_ref().and_then(|project| {
            self.selected_chart_id
                .as_ref()
                .and_then(|chart_id| project.charts_by_id.get(chart_id))
                .cloned()
        })
    }

    pub fn get_control_set(&self) -> Option<Arc<ControlSet>> {
        self.get_chart().and_then(|chart| chart.control_set.clone())
    }
}

impl DemoTool {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<DemoTool> {
        let mut resource_repository = ResourceRepository::try_new()?;
        let project = resource_repository.get_or_load_project(context);
        let ui = Ui::new(context, event_loop)?;

        let ui_state = UiState {
            time: 5.0,
            is_playing: false,
            selected_chart_id: None,
            project,
        };

        let demo_tool = DemoTool {
            ui,
            start_time: Instant::now(),
            resource_repository,
            ui_state,
            has_render_failure: project.is_none(),
            play_start_time: Instant::now(),
            last_eval_time: -1.0,
        };
        Ok(demo_tool)
    }

    pub fn draw(&mut self, context: &mut RenderContext, chart: &Chart) -> Result<()> {
        let elapsed = self.start_time.elapsed().as_secs_f32();

        if self.ui_state.is_playing {
            self.ui_state.time = self.play_start_time.elapsed().as_secs_f32();
        }

        // Evaluate control splines
        if self.last_eval_time != self.ui_state.time {
            self.last_eval_time = self.ui_state.time;
            for control in &mut self
                .resource_repository
                .control_repository
                .used_controls_list
            {
                control.evaluate_splines(self.ui_state.time);
            }
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

        chart.render(context)
    }
}

impl VulkanApp for DemoTool {
    fn paint(&mut self, vulkan_context: &VulkanContext, renderer: &mut VulkanoWindowRenderer) {
        let Some(project) = self
            .resource_repository
            .get_or_load_project(vulkan_context) else {
            return;
        };

        if let Some(last_project) = &self.ui_state.project {
            // If the last loaded document is not the same as the current one
            if !Arc::ptr_eq(last_project, &project) {
                self.has_render_failure = false;
                self.ui_state.project = Some(project.clone());
            }
        } else {
            // If there was no last loaded document
            self.has_render_failure = false;
            self.ui_state.project = Some(project.clone());
        }

        let before_future = renderer.acquire().unwrap();
        let target_image = renderer.swapchain_image_view();
        let depth_image = renderer.get_additional_image_view(1);

        let scale_factor = renderer.window().scale_factor() as f32;

        let size = target_image.dimensions();
        let movie_height = (size.width() * 9 / 16) as i32;
        let ui_height = max(size.height() as i32 - movie_height, 0) as f32 / scale_factor;
        let draw_ui = ui_height > 0.0;

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

            // If the last render failed, stop rendering until the user changes the document
            if !self.has_render_failure {
                if let Some(project) = &self.ui_state.project {
                    if let Some(chart_id) = &self.ui_state.selected_chart_id {
                        if let Some(chart) = project.charts.get(&chart_id) {
                            if let Err(err) = self.draw(&mut context, chart) {
                                error!("Render failed: {}", err);
                                self.has_render_failure = true;
                            }
                        }
                    }
                }
            }

            let is_playing = self.ui_state.is_playing;

            // Render UI
            if draw_ui {
                self.ui.draw(
                    &mut context,
                    ui_height,
                    &mut self.resource_repository.control_repository,
                    &mut self.ui_state,
                );
            }

            // Start playing
            if self.ui_state.is_playing && !is_playing {
                self.play_start_time = Instant::now() - Duration::from_secs_f32(self.ui_state.time);
            }
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
