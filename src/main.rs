use std::sync::Arc;
use std::time::Instant;

use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

fn create_test_mesh(device: &wgpu::Device) -> Mesh {
    let verts = [
        Vertex {
            pos: [0.0, 0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.5, 0.0],
        },
        Vertex {
            pos: [-0.5, -0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
        },
        Vertex {
            pos: [0.5, -0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
        },
    ];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let indices = [0, 1, 2];
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    println!("VERTICES: {:?}", &verts[..3]);
    println!("INDICES: {:?}", &indices[..3]);

    Mesh {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    }
}

fn load_gltf(device: &wgpu::Device, path: &str) -> Vec<Mesh> {
    let (doc, buffs, _) = gltf::import(path).unwrap();
    let mut meshes = vec![];

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            // ─── vertices (POS + NORMAL + UV) ───
            let reader = prim.reader(|b| Some(&buffs[b.index()]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .map(|v| v.collect())
                .unwrap_or_else(|| vec![]);
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|v| v.collect())
                .unwrap_or_else(|| vec![[0.0; 3]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|v| v.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0; 2]; positions.len()]);

            let verts: Vec<Vertex> = positions
                .iter()
                .enumerate()
                .map(|(i, &pos)| Vertex {
                    pos,
                    normal: normals.get(i).copied().unwrap_or([0.0; 3]),
                    uv: uvs.get(i).copied().unwrap_or([0.0; 2]),
                })
                .collect();

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

            // ─── indices ───
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|v| v.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            println!("VERTICES: {:?}", &verts[..3]);
            println!("INDICES: {:?}", &indices[..3]);

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(Mesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
            });
        }
    }
    meshes
}

struct State {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    render_pipeline: wgpu::RenderPipeline,
    meshes: Vec<Mesh>,
    camera: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    start_time: Instant,
    depth_texture_view: wgpu::TextureView,
}

impl State {
    async fn new(window: Arc<Window>) -> State {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let size = window.inner_size();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        let camera = CameraUniform {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: Some("depth_texture"),
            view_formats: &[],
        });

        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let model_vert = std::fs::read("shaders/model.vert.spv").unwrap();
        let model_frag = std::fs::read("shaders/model.frag.spv").unwrap();

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::SpirV(bytemuck::cast_slice(&model_vert).into()),
                }),
                entry_point: Some("vsMain"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::SpirV(bytemuck::cast_slice(&model_frag).into()),
                }),
                entry_point: Some("psMain"),
                compilation_options: Default::default(),
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let mut meshes = load_gltf(&device, "models/Fox.gltf");
        // let mut meshes = vec![];
        // meshes.push(create_test_mesh(&device));

        let start_time = Instant::now();

        let state = State {
            window,
            device,
            queue,
            size,
            surface,
            surface_format,
            render_pipeline,
            meshes,
            camera,
            camera_buffer,
            camera_bind_group,
            start_time,
            depth_texture_view,
        };
        state.configure_surface();

        state
    }

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(&self.device, &surface_config);
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
    }

    fn render(&mut self) {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_format.add_srgb_suffix()),
                ..Default::default()
            });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut renderpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let elapsed = self.start_time.elapsed().as_secs_f32();

        let aspect_ratio = self.size.width as f32 / self.size.height as f32;
        let radius = 150.0;
        let height = 40.0;
        let camera_position = glam::vec3(elapsed.sin() * radius, height, elapsed.cos() * radius);
        let target = glam::Vec3::ZERO;
        let up = glam::Vec3::Y;

        let view = glam::Mat4::look_at_rh(camera_position, target, up);
        let projection =
            glam::Mat4::perspective_rh_gl(70.0_f32.to_radians(), aspect_ratio, 0.1, 1000.0);

        self.camera.view_proj = (projection * view).to_cols_array_2d();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera]));

        renderpass.set_pipeline(&self.render_pipeline);
        renderpass.set_bind_group(0, &self.camera_bind_group, &[]);
        for mesh in &self.meshes {
            renderpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            renderpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            renderpass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }

        drop(renderpass);

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let state = pollster::block_on(State::new(window.clone()));
        self.state = Some(state);

        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                state.render();
                state.get_window().request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.resize(size);
            }
            _ => (),
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
