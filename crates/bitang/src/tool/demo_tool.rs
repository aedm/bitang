use crate::control::controls::{ControlRepository, ControlSet};
use crate::control::{ControlId, ControlIdPartType};
use crate::file::resource_repository::ResourceRepository;
use crate::render::chart::Chart;
use crate::render::project::Project;
use crate::render::vulkan_window::{
    PaintResult, RenderContext, VulkanApp, VulkanContext, FRAMEDUMP_FPS, FRAMEDUMP_HEIGHT,
    FRAMEDUMP_MODE, FRAMEDUMP_WIDTH,
};
use crate::tool::music_player::MusicPlayer;
use crate::tool::ui::Ui;
use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::cmp::max;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::{error, info};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo,
    PrimaryCommandBufferAbstract,
};
use vulkano::image::ImageViewAbstract;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::VulkanoWindowRenderer;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

const SCREEN_RATIO: (u32, u32) = (16, 9);

pub struct DemoTool {
    ui: Ui,
    start_time: Instant,
    resource_repository: ResourceRepository,
    has_render_failure: bool,
    ui_state: UiState,
    play_start_time: Instant,
    last_eval_time: f32,
    music_player: MusicPlayer,
    is_fullscreen: bool,

    dumped_frame_buffer: Option<Subbuffer<[u8]>>,
    frame_counter: usize,
}

pub struct UiState {
    pub project: Option<Arc<Project>>,
    pub selected_control_id: ControlId,
    pub time: f32,
    pub is_playing: bool,
    pub control_repository: Rc<RefCell<ControlRepository>>,
}

impl UiState {
    pub fn get_chart(&self) -> Option<Rc<Chart>> {
        let id_first = self.selected_control_id.parts.first();
        if let Some(project) = &self.project {
            if let Some(id_first) = id_first {
                if id_first.part_type == ControlIdPartType::Chart {
                    return project.charts_by_id.get(&id_first.name).cloned();
                }
            }
        }
        None
    }

    pub fn get_current_chart_control_set(&self) -> Option<Rc<ControlSet>> {
        self.get_chart()
            .and_then(|chart| Some(chart.controls.clone()))
    }

    pub fn get_time(&self) -> f32 {
        if let Some(part) = self.selected_control_id.parts.first() {
            if let Some(project) = &self.project {
                if part.part_type == ControlIdPartType::Chart {
                    if let Some(time) = project
                        .cuts
                        .iter()
                        .find(|cut| cut.chart.id == part.name)
                        .map(|cut| cut.start_time)
                    {
                        return time + self.time;
                    }
                }
            }
        }
        return self.time;
    }
}

impl DemoTool {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<DemoTool> {
        let music_player = MusicPlayer::new();

        let mut resource_repository = ResourceRepository::try_new()?;
        let project = resource_repository.get_or_load_project(context);
        let ui = Ui::new(context, event_loop)?;
        let has_render_failure = project.is_none();

        let ui_state = UiState {
            time: 0.0,
            is_playing: false,
            project,
            control_repository: resource_repository.control_repository.clone(),
            selected_control_id: ControlId::default(),
        };

        let dumped_frame_buffer = if FRAMEDUMP_MODE {
            let buffer = Buffer::from_iter(
                context.context.memory_allocator(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    usage: MemoryUsage::Download,
                    ..Default::default()
                },
                (0..FRAMEDUMP_WIDTH * FRAMEDUMP_HEIGHT * 4).map(|_| 0u8),
            )?;
            Some(buffer)
        } else {
            None
        };

        let demo_tool = DemoTool {
            ui,
            start_time: Instant::now(),
            resource_repository,
            ui_state,
            has_render_failure,
            play_start_time: Instant::now(),
            last_eval_time: -1.0,
            music_player,
            dumped_frame_buffer,
            frame_counter: 0,
            is_fullscreen: false,
        };
        Ok(demo_tool)
    }

    pub fn draw(&mut self, context: &mut RenderContext) -> Result<()> {
        match self.ui_state.get_chart() {
            Some(chart) => self.draw_chart(&chart, context),
            None => self.draw_project(context),
        }
    }

    fn draw_chart(&mut self, chart: &Chart, context: &mut RenderContext) -> Result<()> {
        // Evaluate control splines
        let should_evaluate = true;
        if should_evaluate {
            if let Some(control_set) = self.ui_state.get_current_chart_control_set() {
                self.last_eval_time = self.ui_state.time;
                for control in &control_set.used_controls {
                    control.evaluate_splines(self.ui_state.time);
                }
            }
        }
        context.globals.chart_time = self.ui_state.time;
        chart.render(context)
    }

    fn draw_project(&mut self, context: &mut RenderContext) -> Result<()> {
        let Some(project) = &self.ui_state.project else {
            return Err(anyhow!("No project loaded"));
        };

        // Evaluate control splines and draw charts
        let time = self.ui_state.time;
        for cut in &project.cuts {
            if cut.start_time <= time && time <= cut.end_time {
                let chart_time = time - cut.start_time + cut.offset;
                for control in &cut.chart.controls.used_controls {
                    control.evaluate_splines(chart_time);
                }
                context.globals.chart_time = chart_time;
                cut.chart.render(context)?
            }
        }
        Ok(())
    }

    fn toggle_play(&mut self) {
        if self.ui_state.is_playing {
            self.stop();
        } else {
            self.play();
        }
    }

    fn copy_frame_to_buffer(&mut self, render_context: &mut RenderContext) {
        let buf = self.dumped_frame_buffer.as_ref().unwrap();
        let image = render_context
            .vulkan_context
            .swapchain_render_targets_by_id
            .get("screen")
            .unwrap()
            .image
            .borrow()
            .as_ref()
            .unwrap()
            .texture
            .as_ref()
            .unwrap()
            .clone();
        render_context
            .command_builder
            .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(image, buf.clone()))
            .unwrap();
    }

    fn get_frame_content(&mut self) -> Vec<u8> {
        let buf = self.dumped_frame_buffer.as_ref().unwrap();
        let buffer_lock = buf.read().unwrap();
        let content: Vec<u8> = buffer_lock.to_vec();
        content
    }

    fn save_frame_buffer_to_file(content: Vec<u8>, frame_number: usize) {
        let path = format!("framedump/dump-{:0>8}.png", frame_number);
        let save_timer = Instant::now();
        image::save_buffer_with_format(
            &path,
            &content,
            FRAMEDUMP_WIDTH,
            FRAMEDUMP_HEIGHT,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        info!(
            "Saved frame {} to {} ({}ms)",
            frame_number,
            path,
            save_timer.elapsed().as_millis()
        );
    }

    fn render_frame_to_screen(
        &mut self,
        vulkan_context: &VulkanContext,
        renderer: &mut VulkanoWindowRenderer,
    ) -> PaintResult {
        // Update swapchain targets
        let before_future = renderer.acquire().unwrap();
        let target_image = renderer.swapchain_image_view();
        let depth_image = renderer.get_additional_image_view(1);

        vulkan_context
            .swapchain_render_targets_by_id
            .get("screen")
            .unwrap()
            .update_swapchain_image(target_image.clone());
        vulkan_context
            .swapchain_render_targets_by_id
            .get("screen_depth")
            .unwrap()
            .update_swapchain_image(depth_image);

        // Calculate viewport
        let window_size = target_image.dimensions();
        let scale_factor = renderer.window().scale_factor() as f32;
        let (width, height, top, left) = if self.is_fullscreen {
            if window_size.width() * SCREEN_RATIO.1 > window_size.height() * SCREEN_RATIO.0 {
                // Screen is too wide
                let height = window_size.height();
                let width = height * SCREEN_RATIO.0 / SCREEN_RATIO.1;
                let left = (window_size.width() - width) / 2;
                let top = 0;
                (width, height, top, left)
            } else {
                // Screen is too tall
                let width = window_size.width();
                let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
                let left = 0;
                let top = (window_size.height() - height) / 2;
                (width, height, top, left)
            }
        } else {
            let width = window_size.width();
            let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
            let left = 0;
            let top = 0;
            (width, height, top, left)
        };
        let ui_height = max(window_size.height() as i32 - height as i32, 0) as f32 / scale_factor;
        let screen_viewport = Viewport {
            origin: [left as f32, top as f32],
            dimensions: [width as f32, height as f32],
            depth_range: 0.0..1.0,
        };

        // Make command buffer
        let mut command_builder = AutoCommandBufferBuilder::primary(
            &vulkan_context.command_buffer_allocator,
            vulkan_context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // Make render context
        let mut context = RenderContext {
            vulkan_context,
            screen_buffer: target_image,
            screen_viewport,
            command_builder: &mut command_builder,
            globals: Default::default(),
        };

        // Render content
        let before_time = self.ui_state.time;
        if self.ui_state.is_playing {
            self.ui_state.time = self.play_start_time.elapsed().as_secs_f32();
        }
        context.globals.app_time = self.start_time.elapsed().as_secs_f32();

        let mut paint_result = PaintResult::None;
        let project = self.issue_render_commands(&mut context);

        if let Some(project) = project {
            if before_time < project.length && self.ui_state.time >= project.length {
                paint_result = PaintResult::EndReached;
            }
        }

        // Render UI
        if !self.is_fullscreen && ui_height > 0.0 {
            self.ui.draw(&mut context, ui_height, &mut self.ui_state);
        }

        // Execute commands and display the result
        let command_buffer = command_builder.build().unwrap();
        let after_future = before_future
            .then_execute(
                vulkan_context.context.graphics_queue().clone(),
                command_buffer,
            )
            .unwrap()
            .boxed();
        renderer.present(after_future, true);

        paint_result
    }

    fn render_frame_to_file(&mut self, vulkan_context: &VulkanContext) -> Arc<Project> {
        let target_image = vulkan_context
            .swapchain_render_targets_by_id
            .get("screen")
            .unwrap()
            .image
            .borrow()
            .as_ref()
            .unwrap()
            .image_view
            .clone();
        let size = target_image.dimensions();
        let screen_viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [size.width() as f32, size.height() as f32],
            depth_range: 0.0..1.0,
        };

        // Make command buffer
        let mut command_builder = AutoCommandBufferBuilder::primary(
            &vulkan_context.command_buffer_allocator,
            vulkan_context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // Render content
        let mut context = RenderContext {
            vulkan_context,
            screen_buffer: target_image,
            screen_viewport,
            command_builder: &mut command_builder,
            globals: Default::default(),
        };
        context.globals.app_time = self.ui_state.time;

        let project = self.issue_render_commands(&mut context).unwrap();

        // Add a copy command to the end of the command buffer
        self.copy_frame_to_buffer(&mut context);

        // Execute commands on the graphics queue and dump it to a file
        let command_buffer = command_builder.build().unwrap();
        let queue = vulkan_context.context.graphics_queue().clone();
        let finished = command_buffer.execute(queue).unwrap();
        finished
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();

        project
    }

    fn issue_render_commands(&mut self, context: &mut RenderContext) -> Option<Arc<Project>> {
        let Some(project) = self
            .resource_repository
            .get_or_load_project(context.vulkan_context) else {
            return None;
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

        if let Err(err) = self.draw(context) {
            if !self.has_render_failure {
                error!("Render failed: {:?}", err);
                self.has_render_failure = true;
            }
        } else {
            self.has_render_failure = false;
        }

        Some(project)
    }

    fn render_demo_to_file(&mut self, vulkan_context: &VulkanContext) {
        let timer = Instant::now();
        // PNG compression is slow, so let's use all the CPU cores
        rayon::in_place_scope(|scope| {
            let job_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
            loop {
                self.ui_state.time = self.frame_counter as f32 / (FRAMEDUMP_FPS as f32);

                // Render frame and save it into host memory
                let project = self.render_frame_to_file(vulkan_context);
                let content = self.get_frame_content();

                // If we're rendering too fast, wait a bit
                while job_count.load(Ordering::Relaxed) >= rayon::current_num_threads() + 10 {
                    sleep(Duration::from_millis(1));
                }

                // Save the frame to a file in a separate thread
                job_count.fetch_add(1, Ordering::Relaxed);
                let frame_number = self.frame_counter;
                let job_count_clone = job_count.clone();
                scope.spawn(move |_| {
                    Self::save_frame_buffer_to_file(content, frame_number);
                    job_count_clone.fetch_sub(1, Ordering::Relaxed);
                });

                if self.ui_state.time >= project.length {
                    return;
                }
                self.frame_counter += 1;
            }
        });
        info!(
            "Rendered {} frames in {:.1} secs",
            self.frame_counter,
            timer.elapsed().as_secs_f32()
        );
    }
}

impl VulkanApp for DemoTool {
    fn paint(
        &mut self,
        vulkan_context: &VulkanContext,
        renderer: &mut VulkanoWindowRenderer,
    ) -> PaintResult {
        if FRAMEDUMP_MODE {
            self.render_demo_to_file(vulkan_context);
            return PaintResult::EndReached;
        };

        self.render_frame_to_screen(vulkan_context, renderer)
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        self.ui.handle_window_event(event);
        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                if input.state == winit::event::ElementState::Pressed {
                    match input.virtual_keycode {
                        Some(winit::event::VirtualKeyCode::Space) => {
                            self.toggle_play();
                        }
                        _ => (),
                    }
                }
            }
            _ => {}
        }
    }

    fn play(&mut self) {
        self.music_player.play_from(self.ui_state.get_time());
        let now = Instant::now();
        // Duration is always positive
        if self.ui_state.time >= 0. {
            self.play_start_time = now - Duration::from_secs_f32(self.ui_state.time);
        } else {
            self.play_start_time = now + Duration::from_secs_f32(-self.ui_state.time);
        }
        self.ui_state.is_playing = true;
    }

    fn stop(&mut self) {
        self.music_player.stop();
        self.ui_state.is_playing = false;
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.is_fullscreen = fullscreen;
    }
}
