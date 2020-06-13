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
    bind_group: wgpu::BindGroup,
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

        let surface = wgpu::Surface::create(&parent.raw_handle());
        let adapter = wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY,
        )
        .await
        .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                extensions: wgpu::Extensions {
                    anisotropic_filtering: false,
                },
                limits: wgpu::Limits::default(),
            })
            .await;

        let vs = include_bytes!("quad.spv");
        let vs_module =
            device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&vs[..])).unwrap());

        let fs = include_bytes!("dummy_wp.spv");
        let fs_module =
            device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&fs[..])).unwrap());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[],
            label: None,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: &[],
            label: None,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::Back,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
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
            bind_group,
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
            .get_next_texture()
            .expect("Timeout when acquiring next swap chain texture");

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::BLACK,
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(&[encoder.finish()]);
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
