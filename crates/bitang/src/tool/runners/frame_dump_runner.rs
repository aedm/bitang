use anyhow::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::info;

use crate::tool::content_renderer::ContentRenderer;
use crate::engine::{
    FrameContext, GpuContext, Viewport
};
use crate::tool::{FRAMEDUMP_FPS, FRAMEDUMP_HEIGHT, FRAMEDUMP_WIDTH};

pub struct FrameDumpRunner {
    gpu_context: Arc<GpuContext>,
    app: ContentRenderer,
    dst_buffer: wgpu::Buffer,
}

impl FrameDumpRunner {
    pub fn run() -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        let gpu_context = rt.block_on(async { GpuContext::new_for_offscreen().await })?;
        let gpu_context = Arc::new(gpu_context);
        let mut app = ContentRenderer::new(&gpu_context)?;
        app.reset_simulation(&gpu_context)?;

        let dst_buffer = gpu_context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Image save buffer"),
            size: (FRAMEDUMP_WIDTH * FRAMEDUMP_HEIGHT * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut frame_dump_runner = Self {
            gpu_context,
            app,
            dst_buffer,
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
        // Fix the alpha channel
        for i in 0..content.len() / 4 {
            content[i * 4 + 3] = 255;
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
        let size = [FRAMEDUMP_WIDTH, FRAMEDUMP_HEIGHT];
        let screen_viewport = Viewport { x: 0, y: 0, size };
        self.gpu_context.final_render_target.enforce_size_rule(&self.gpu_context, &size).unwrap();

        let command_encoder = self
            .gpu_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            command_encoder,
            globals: Default::default(),
            screen_viewport,
        };
        frame_context.globals.simulation_elapsed_time_since_last_render =
            1.0 / (FRAMEDUMP_FPS as f32);
        frame_context.globals.app_time = self.app.app_state.cursor_time;

        self.app.draw(&mut frame_context);

        // Add a copy command to the end of the command buffer
        self.gpu_context
            .final_render_target
            .copy_attachment_to_buffer(&mut frame_context, &self.dst_buffer)
            .unwrap();

        self.gpu_context.queue.submit(Some(frame_context.command_encoder.finish()));
    }

    fn get_frame_content(&mut self) -> Vec<u8> {
        let res = {
            let dst_buffer_slice = self.dst_buffer.slice(..);
            dst_buffer_slice.map_async(wgpu::MapMode::Read, |_| ());
            self.gpu_context.device.poll(wgpu::Maintain::Wait);
            dst_buffer_slice.get_mapped_range().to_vec()
        };
        self.dst_buffer.unmap();
        res
    }
}
