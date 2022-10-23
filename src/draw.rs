use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::render_pass::Framebuffer;
use vulkano_win::VkSurfaceBuild;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

pub const NUMBER_FRAMES_IN_FLIGHT: usize = 3;

pub struct VulkanRenderer {
    current_frame: usize,
    framebuffers: Vec<Framebuffer>,
}

impl VulkanRenderer {
    fn run() {

    }
}

pub fn main() {
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
                .find(|&q| q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false))
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
}