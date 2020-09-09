use std::sync::Arc;
use wstk::*;

pub struct Wallpaper {
    parent: DesktopInstance,
    scale: i32,
    width: u32,
    height: u32,

    device: wgpu::Device,
    surface: wgpu::Surface,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    swap_chain: Option<wgpu::SwapChain>,
}

struct WallpaperSetup;
impl DesktopSurface for WallpaperSetup {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>) {
        layer_surface.set_layer(layer_shell::Layer::Background);
        layer_surface.set_anchor(
            layer_surface::Anchor::Left
                | layer_surface::Anchor::Top
                | layer_surface::Anchor::Right
                | layer_surface::Anchor::Bottom,
        );
        layer_surface.set_size(0, 0);
    }
}

impl Wallpaper {
    pub async fn new(env: Environment<Env>, display: Display, queue: &EventQueue) -> Self {
        let parent = DesktopInstance::new(&WallpaperSetup, env, display, queue);

        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(&parent.raw_handle()) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    shader_validation: false,
                },
                None,
            )
            .await
            .unwrap();

        let vs_module = device.create_shader_module(wgpu::include_spirv!("quad.spv"));

        let fs_module = device.create_shader_module(wgpu::include_spirv!("dummy_wp.spv"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: None, // default
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        parent.wl_surface.commit();
        parent.flush();

        Wallpaper {
            parent,
            scale: 1,
            width: 0,
            height: 0,
            device,
            surface,
            queue,
            render_pipeline,
            swap_chain: None,
        }
    }

    fn create_swap_chain(&mut self) {
        self.swap_chain = Some(self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                width: self.width * self.scale as u32,
                height: self.height * self.scale as u32,
                present_mode: wgpu::PresentMode::Mailbox,
            },
        ));

        self.parent.wl_surface.set_buffer_scale(self.scale);
    }

    fn render(&mut self) {
        let frame = self
            .swap_chain
            .as_mut()
            .unwrap()
            .get_current_frame()
            .expect("Timeout when acquiring next swap chain texture")
            .output;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
    }

    async fn on_scale(&mut self, scale: i32) {
        if scale == self.scale {
            return;
        }
        self.scale = scale;
        self.create_swap_chain();
        self.render();
    }

    async fn on_layer_event(&mut self, event: Arc<layer_surface::Event>) {
        match &*event {
            layer_surface::Event::Configure {
                ref serial,
                ref width,
                ref height,
            } => {
                self.parent.layer_surface.ack_configure(*serial);

                self.scale = get_surface_scale_factor(&self.parent.wl_surface);
                self.width = *width;
                self.height = *height;
                self.create_swap_chain();
                self.render();
            }
            _ => eprintln!("todo: lsh close"),
        }
    }

    pub async fn run(&mut self) {
        let mut layer_events = wayland_event_chan(&self.parent.layer_surface);
        loop {
            futures::select! {
                ev = layer_events.next() => if let Some(event) = ev { self.on_layer_event(event).await },
                sc = self.parent.scale_rx.next() => if let Some(scale) = sc { self.on_scale(scale).await },
            }
        }
    }
}
