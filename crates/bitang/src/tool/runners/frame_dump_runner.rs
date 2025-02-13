use crate::render::image::{BitangImage, ImageSizeRule};
use crate::render::{SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
use anyhow::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::info;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo,
    PrimaryCommandBufferAbstract,
};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::GpuFuture;

use crate::tool::content_renderer::ContentRenderer;
use crate::tool::{
    WgpuContext, FrameContext, RenderContext, FRAMEDUMP_FPS, FRAMEDUMP_HEIGHT, FRAMEDUMP_WIDTH,
};

pub struct FrameDumpRunner {
    vulkan_context: Arc<RenderContext>,
    dumped_frame_buffer: Subbuffer<[u8]>,
    app: ContentRenderer,
}

impl FrameDumpRunner {
    pub fn run() -> Result<()> {
        let frame_size = ImageSizeRule::Fixed(FRAMEDUMP_WIDTH, FRAMEDUMP_HEIGHT);
        let final_render_target = BitangImage::new_attachment(
            SCREEN_RENDER_TARGET_ID,
            SCREEN_COLOR_FORMAT,
            frame_size,
            false,
        );

        let vulkan_context = init_context.into_vulkan_context(final_render_target);

        let mut app = ContentRenderer::new(&vulkan_context)?;
        app.reset_simulation(&vulkan_context)?;

        // TODO: Buffer::new?
        let dumped_frame_buffer = Buffer::from_iter(
            vulkan_context.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            (0..FRAMEDUMP_WIDTH * FRAMEDUMP_HEIGHT * 4).map(|_| 0u8),
        )?;

        let mut frame_dump_runner = Self {
            vulkan_context,
            dumped_frame_buffer,
            app,
        };

        frame_dump_runner.render_demo_to_file();
        Ok(())
    }

    fn render_demo_to_file(&mut self) {
        let timer = Instant::now();
        // PNG compression is slow, so let's use all the CPU cores
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let job_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let cpu_count = num_cpus::get();
        info!("Rendering demo using {cpu_count} CPUs");
        let project_length = self.app.app_state.project.as_ref().unwrap().length;
        let mut frame_count = 0;

        loop {
            let time = frame_count as f32 / (FRAMEDUMP_FPS as f32);
            if time >= project_length {
                break;
            }
            self.app.app_state.set_time(time);

            // Render frame and save it into host memory
            self.render_frame_to_buffer();
            let content = self.get_frame_content();

            // If we're rendering too fast, wait a bit
            while job_count.load(Ordering::Relaxed) >= cpu_count + 20 {
                sleep(Duration::from_millis(1));
            }

            // Save the frame to a file in a separate thread
            job_count.fetch_add(1, Ordering::Relaxed);
            let job_count_clone = job_count.clone();
            runtime.spawn_blocking(move || {
                Self::save_frame_buffer_to_file(content, frame_count);
                job_count_clone.fetch_sub(1, Ordering::Relaxed);
            });
            frame_count += 1;
        }
        info!(
            "Rendered {frame_count} frames in {:.1} secs",
            timer.elapsed().as_secs_f32()
        );
    }

    fn save_frame_buffer_to_file(mut content: Vec<u8>, frame_number: usize) {
        for i in 0..content.len() / 4 {
            // Fix the alpha channel
            content[i * 4 + 3] = 255;

            // Fix the RGB order
            content.swap(i * 4, i * 4 + 2);
        }

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
            "Saved frame {frame_number} to {path} ({}ms)",
            save_timer.elapsed().as_millis()
        );
    }

    fn render_frame_to_buffer(&mut self) {
        let size = match self.vulkan_context.final_render_target.size_rule {
            ImageSizeRule::Fixed(w, h) => [w, h],
            _ => panic!("Screen render target must have a fixed size"),
        };
        let screen_viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [size[0] as f32, size[1] as f32],
            depth_range: 0.0..=1.0,
        };
        self.vulkan_context
            .final_render_target
            .enforce_size_rule(&self.vulkan_context, screen_viewport.extent)
            .unwrap();

        // Make command buffer
        let mut command_builder = AutoCommandBufferBuilder::primary(
            &self.vulkan_context.command_buffer_allocator,
            self.vulkan_context.gfx_queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // Render content
        let mut render_context = FrameContext {
            vulkan_context: self.vulkan_context.clone(),
            screen_viewport,
            command_builder: &mut command_builder,
            globals: Default::default(),
            simulation_elapsed_time_since_last_render: 1.0 / (FRAMEDUMP_FPS as f32),
        };
        render_context.globals.app_time = self.app.app_state.cursor_time;

        self.app.draw(&mut render_context);

        // Add a copy command to the end of the command buffer
        self.add_frame_to_buffer_copy_command(&mut render_context);

        // Execute commands on the graphics queue
        let command_buffer = command_builder.build().unwrap();
        let queue = self.vulkan_context.gfx_queue.clone();
        let finished = command_buffer.execute(queue).unwrap();
        finished
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();
    }

    fn get_frame_content(&mut self) -> Vec<u8> {
        let buffer_lock = self.dumped_frame_buffer.read().unwrap();
        buffer_lock.to_vec()
    }

    fn add_frame_to_buffer_copy_command(&mut self, render_context: &mut FrameContext) {
        let image = render_context
            .vulkan_context
            .final_render_target
            .get_image();
        render_context
            .command_builder
            .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                image,
                self.dumped_frame_buffer.clone(),
            ))
            .unwrap();
    }
}
