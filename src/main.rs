use std::collections::HashMap;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct ViewportDesc {
    window: Window,
    background: wgpu::Color,
    surface: wgpu::Surface,
}

struct Viewport {
    desc: ViewportDesc,
    config: wgpu::SurfaceConfiguration,
}

impl ViewportDesc {
    fn new(window: Window, background: wgpu::Color, instance: &wgpu::Instance) -> Self {
        let surface = unsafe { instance.create_surface(&window) };
        Self {
            window,
            background,
            surface,
        }
    }

    fn build(self, adapter: &wgpu::Adapter, device: &wgpu::Device) -> Viewport {
        let size = self.window.inner_size();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface.get_supported_formats(adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };

        self.surface.configure(device, &config);

        Viewport { desc: self, config }
    }
}

impl Viewport {
    fn resize(&mut self, device: &wgpu::Device, size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = size.width;
        self.config.height = size.height;
        self.desc.surface.configure(device, &self.config);
    }
    fn get_current_texture(&mut self) -> wgpu::SurfaceTexture {
        self.desc
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture")
    }
}

async fn run(event_loop: EventLoop<()>, viewport: (Window, wgpu::Color)) {
    let instance = wgpu::Instance::new(wgpu::Backends::all());

    let (window, color) = viewport;
    let desc = ViewportDesc::new(window, color, &instance);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            // Request an adapter which can render to our surface
            compatible_surface: Some(&desc.surface),
            ..Default::default()
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    let mut viewports: HashMap<WindowId, Viewport> = vec![desc]
        .into_iter()
        .map(|desc| (desc.window.id(), desc.build(&adapter, &device)))
        .collect();

    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter);

        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Recreate the swap chain with the new size
                if let Some(viewport) = viewports.get_mut(&window_id) {
                    viewport.resize(&device, size);
                    // On macos the window needs to be redrawn manually after resizing
                    viewport.desc.window.request_redraw();
                }
            }
            Event::RedrawRequested(window_id) => {
                if let Some(viewport) = viewports.get_mut(&window_id) {
                    let frame = viewport.get_current_texture();
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    {
                        let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(viewport.desc.background),
                                    store: true,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });
                    }

                    queue.submit(Some(encoder.finish()));
                    frame.present();
                }
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                viewports.remove(&window_id);
                if viewports.is_empty() {
                    *control_flow = ControlFlow::Exit
                }
            }
            _ => {}
        }
    });
}

fn main() {
    const WINDOW_SIZE: u32 = 128;

    let event_loop = EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("minimg")
        .with_inner_size(winit::dpi::PhysicalSize::new(WINDOW_SIZE, WINDOW_SIZE))
        .build(&event_loop)
        .unwrap();

    fn frac(index: u32, max: u32) -> f64 {
        index as f64 / max as f64
    }

    let viewport = (
        window,
        wgpu::Color {
            r: frac(0, 4),
            g: 0.5 - frac(0, 16) * 0.5,
            b: frac(0, 4),
            a: 1.0,
        },
    );

    env_logger::init();
    // Temporarily avoid srgb formats for the swapchain on the web
    pollster::block_on(run(event_loop, viewport));
}
