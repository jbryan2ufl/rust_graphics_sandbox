use crate::mesh::load_gltf;
use crate::{egui_renderer::EguiRenderer, mesh::Mesh};
use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use crossbeam::queue::SegQueue;
use egui_wgpu::ScreenDescriptor;
use std::sync::RwLock;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use wgpu::util::BufferInitDescriptor;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{
    camera::{Camera, MainCamera},
    material::{Binding, Material},
    shader::Shader,
};

use wgpu_profiler::GpuProfiler;

#[derive(Resource)]
struct GpuProfilerResource {
    profiler: GpuProfiler,
}

struct GpuCreateBufferCommand {
    data: Arc<Vec<u8>>,
    desc: BufferInitDescriptor<'static>,
}
struct GpuWriteBufferCommand {
    pub buffer: wgpu::Buffer,
    pub offset: wgpu::BufferAddress,
    pub data: Vec<u8>,
}

impl GpuWriteBufferCommand {
    pub fn new<T: bytemuck::Pod>(
        buffer: wgpu::Buffer,
        offset: wgpu::BufferAddress,
        data: &T,
    ) -> Self {
        Self {
            buffer,
            offset,
            data: bytemuck::cast_slice(&[*data]).to_vec(),
        }
    }
}

struct GpuRenderCommand {
    pipeline: Arc<wgpu::RenderPipeline>,
    bind_groups: Arc<Vec<wgpu::BindGroup>>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[derive(Resource, Default)]
struct GpuWriteBufferCommandQueue {
    queue: Arc<SegQueue<GpuWriteBufferCommand>>,
}

#[derive(Resource, Default)]
struct GpuCreateBufferCommandQueue {
    queue: Arc<SegQueue<GpuCreateBufferCommand>>,
}

#[derive(Resource, Default)]
struct GpuRenderCommandQueue {
    queue: Arc<SegQueue<GpuRenderCommand>>,
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
struct RenderSchedule;
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
struct StartupSchedule;

#[derive(Resource)]
pub struct CommandEncoderResource(wgpu::CommandEncoder);

pub struct Time {
    pub startup: Instant,
    pub delta_seconds: f32,
    pub elapsed_seconds: f32,
    pub last_frame: Instant,
    pub smooth_frametime: f32,
    pub smooth_alpha: f32,
}

impl Default for Time {
    fn default() -> Self {
        let now = Instant::now();
        Time {
            startup: now,
            delta_seconds: 0.0,
            elapsed_seconds: 0.0,
            last_frame: now,
            smooth_frametime: 0.0,
            smooth_alpha: 0.05,
        }
    }
}

impl Time {
    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_millis() as f32;
        self.elapsed_seconds = self.startup.elapsed().as_secs_f32();
        self.last_frame = now;
        self.smooth_frametime =
            self.smooth_alpha * dt + (1.0 - self.smooth_alpha) * self.smooth_frametime;
    }
}

#[derive(Component)]
pub struct Renderable {
    pub mesh: Arc<Mesh>,
    pub material: Arc<Material>,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Handle(usize);

#[derive(Message)]
struct SpawnGltf {
    mesh_handle: Handle,
    material_handle: Handle,
}

#[derive(Resource)]
struct AssetManager<T> {
    assets: RwLock<HashMap<Handle, Arc<T>>>,
    next_handle: RwLock<usize>,
}
impl<T> AssetManager<T> {
    pub fn new() -> Self {
        Self {
            assets: RwLock::new(HashMap::new()),
            next_handle: RwLock::new(0),
        }
    }

    pub fn insert(&self, asset: Arc<T>) -> Handle {
        let handle = {
            let mut next_handle = self.next_handle.write().unwrap();
            let handle = Handle(*next_handle);
            *next_handle += 1;
            handle
        };

        let mut assets = self.assets.write().unwrap();
        assets.insert(handle, asset.clone());

        handle
    }

    pub fn get(&self, handle: Handle) -> Option<Arc<T>> {
        let assets = self.assets.read().unwrap();
        assets.get(&handle).cloned()
    }

    pub fn remove(&self, handle: Handle) -> Option<Arc<T>> {
        let mut assets = self.assets.write().unwrap();
        assets.remove(&handle)
    }
}

fn create_depth_texture(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> DepthTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: None,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    DepthTexture { texture, view }
}

pub struct DepthTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

pub struct State {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface: wgpu::Surface<'static>,
    pub adapter: wgpu::Adapter,
    pub scale_factor: f32,
    pub egui_renderer: EguiRenderer,
    pub depth_texture: DepthTexture,
    pub ecs: World,
    gpu_profiler: wgpu_profiler::GpuProfiler,
}

impl State {
    async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        window: &Window,
        width: u32,
        height: u32,
    ) -> Self {
        let power_pref = wgpu::PowerPreference::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: power_pref,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let features = wgpu::Features::empty();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: features,
                required_limits: Default::default(),
                experimental_features: Default::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let selected_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|d| **d == selected_format)
            .expect("failed to select proper surface texture format!");

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
            desired_maximum_frame_latency: 0,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        let egui_renderer = EguiRenderer::new(&device, surface_config.format, window);

        let scale_factor = 1.0;

        let depth_texture = create_depth_texture(&device, &surface_config);

        let camera = Camera::new(&device, &surface_config);

        let bindings = vec![Binding {
            buffer: camera.buffer.clone(),
            visibility: wgpu::ShaderStages::VERTEX,
        }];

        let shader_manager = AssetManager::<Shader>::new();
        let shader = Arc::new(Shader::new(
            "shaders/model.vert.spv",
            "shaders/model.frag.spv",
        ));
        shader_manager.insert(shader.clone());

        let material_manager = AssetManager::<Material>::new();
        let basic = material_manager.insert(Material::new_arc(
            &device, &surface, &adapter, bindings, &shader,
        ));

        let mesh_manager = AssetManager::<Mesh>::new();
        let mesh_vec = load_gltf(&device, "models/Fox.gltf");
        let mut fox = Handle(0);
        for m in mesh_vec {
            fox = mesh_manager.insert(m);
        }

        let mut ecs = World::default();

        ecs.init_resource::<Messages<SpawnGltf>>();

        ecs.get_resource_or_init::<Schedules>()
            .add_systems(StartupSchedule, spawn_gltf_system)
            .add_systems(RenderSchedule, camera_main_uniform_system)
            .add_systems(RenderSchedule, render_system);

        ecs.spawn((camera, MainCamera));
        ecs.write_message(SpawnGltf {
            mesh_handle: fox,
            material_handle: basic,
        });

        ecs.insert_resource(material_manager);
        ecs.insert_resource(mesh_manager);
        ecs.insert_resource(shader_manager);
        ecs.insert_resource(GpuCreateBufferCommandQueue::default());
        ecs.insert_resource(GpuWriteBufferCommandQueue::default());
        ecs.insert_resource(GpuRenderCommandQueue::default());

        ecs.run_schedule(StartupSchedule);

        let gpu_profiler =
            wgpu_profiler::GpuProfiler::new(&device, wgpu_profiler::GpuProfilerSettings::default())
                .unwrap();

        Self {
            device,
            queue,
            surface,
            surface_config,
            adapter,
            egui_renderer,
            scale_factor,
            depth_texture,
            ecs,
            gpu_profiler,
        }
    }

    fn resize_surface(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        self.depth_texture = create_depth_texture(&self.device, &self.surface_config);
    }
}

pub struct App {
    instance: wgpu::Instance,
    state: Option<State>,
    window: Option<Arc<Window>>,
    screen_descriptor: Option<ScreenDescriptor>,
    time: Time,
}

impl App {
    pub fn new() -> Self {
        let instance = egui_wgpu::wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let time = Time::default();
        Self {
            instance,
            state: None,
            window: None,
            screen_descriptor: None,
            time,
        }
    }

    async fn set_window(&mut self, window: Window) {
        let window = Arc::new(window);
        let width = 1920;
        let height = 1080;

        let _ = window.request_inner_size(PhysicalSize::new(width, height));

        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");

        let state = State::new(&self.instance, surface, &window, width, height).await;

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: window.scale_factor() as f32,
        };

        self.window.get_or_insert(window);
        self.state.get_or_insert(state);
        self.screen_descriptor.get_or_insert(screen_descriptor);
    }

    fn handle_resized(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.state.as_mut().unwrap().resize_surface(width, height);
            self.screen_descriptor = Some(ScreenDescriptor {
                size_in_pixels: [width, height],
                pixels_per_point: self.window.as_ref().unwrap().scale_factor() as f32,
            });
        }
    }

    fn update_and_render(&mut self) {
        let state = self.state.as_mut().unwrap();
        let window = self.window.as_ref().unwrap();

        self.time.update();

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let surface_texture = state.surface.get_current_texture().unwrap();
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut renderpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &state.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            state.ecs.run_schedule(RenderSchedule);

            if let Some(commands) = state.ecs.get_resource_mut::<GpuWriteBufferCommandQueue>() {
                let mut cmd_vec = Vec::new();
                while let Some(cmd) = commands.queue.pop() {
                    cmd_vec.push(cmd);
                }
                for cmd in cmd_vec {
                    state.queue.write_buffer(&cmd.buffer, cmd.offset, &cmd.data);
                }
            }

            if let Some(commands) = state.ecs.get_resource_mut::<GpuRenderCommandQueue>() {
                let mut cmd_vec: Vec<GpuRenderCommand> = Vec::new();
                while let Some(cmd) = commands.queue.pop() {
                    cmd_vec.push(cmd);
                }

                cmd_vec.sort_by_key(|cmd| {
                    let GpuRenderCommand { pipeline, .. } = cmd;
                    Arc::as_ptr(pipeline) as usize
                });

                let mut current_pipeline = None;
                for cmd in &cmd_vec {
                    if current_pipeline
                        .as_ref()
                        .is_none_or(|p| !Arc::ptr_eq(p, &cmd.pipeline))
                    {
                        renderpass.set_pipeline(&cmd.pipeline);
                        current_pipeline = Some(cmd.pipeline.clone());
                    }
                    renderpass.set_bind_group(0, &cmd.bind_groups[0], &[]);
                    renderpass.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
                    renderpass
                        .set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    renderpass.draw_indexed(0..cmd.index_count, 0, 0..1);
                }
            }
        }

        state.egui_renderer.begin_frame(window);
        egui::Window::new("Debug")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(state.egui_renderer.context(), |_ui| {
                _ui.label(format!("Elapsed: {:.2} s", self.time.elapsed_seconds));
                _ui.label(format!("Frametime: {:.2} ms", self.time.smooth_frametime));
                if let Ok(mut cam) = state
                    .ecs
                    .query::<(&mut Camera, &MainCamera)>()
                    .single_mut(&mut state.ecs)
                {
                    if drag_vec3(_ui, "Camera Position", &mut cam.0.eye, 0.1) {
                        cam.0.update_uniform();
                    }
                    _ui.label(format!("{:?}", cam.0));
                }
            });

        state.egui_renderer.end_frame_and_draw(
            &state.device,
            &state.queue,
            &mut encoder,
            window,
            &surface_view,
            self.screen_descriptor.as_ref().unwrap(),
        );

        state.queue.submit(Some(encoder.finish()));
        surface_texture.present();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();
        pollster::block_on(self.set_window(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        self.state
            .as_mut()
            .unwrap()
            .egui_renderer
            .handle_input(self.window.as_ref().unwrap(), &event);

        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.update_and_render();

                self.window.as_ref().unwrap().request_redraw();
            }
            WindowEvent::Resized(new_size) => {
                self.handle_resized(new_size.width, new_size.height);
            }
            _ => (),
        }
    }
}

fn drag_vec3(ui: &mut egui::Ui, label: &str, value: &mut glam::Vec3, speed: f32) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);

        let r_x = ui.add(
            egui::DragValue::new(&mut value.x)
                .speed(speed)
                .prefix("x: "),
        );
        let r_y = ui.add(
            egui::DragValue::new(&mut value.y)
                .speed(speed)
                .prefix("y: "),
        );
        let r_z = ui.add(
            egui::DragValue::new(&mut value.z)
                .speed(speed)
                .prefix("z: "),
        );

        if r_x.changed() || r_y.changed() || r_z.changed() {
            changed = true;
        }
    });

    changed
}

fn camera_main_uniform_system(
    query: Query<&Camera, With<MainCamera>>,
    queue: ResMut<GpuWriteBufferCommandQueue>,
) {
    for camera in query {
        let c = GpuWriteBufferCommand::new(camera.buffer.clone(), 0, &camera.uniform);
        queue.queue.push(c);
    }
}

fn spawn_gltf_system(
    // queue: ResMut<GpuBufferCommandQueue>,
    meshes: Res<AssetManager<Mesh>>,
    materials: Res<AssetManager<Material>>,
    mut messages: MessageReader<SpawnGltf>,
    mut commands: Commands,
) {
    for event in messages.read() {
        if let (Some(mesh), Some(material)) = (
            meshes.get(event.mesh_handle),
            materials.get(event.material_handle),
        ) {
            let renderable = Renderable {
                mesh,
                material,
                visible: true,
            };
            commands.spawn(renderable);
        }
    }
}

fn render_system(query: Query<&Renderable>, queue: ResMut<GpuRenderCommandQueue>) {
    for r in query {
        queue.queue.push(GpuRenderCommand {
            pipeline: r.material.pipeline.clone(),
            bind_groups: r.material.bind_groups.clone(),
            vertex_buffer: r.mesh.vertex_buffer.clone(),
            index_buffer: r.mesh.index_buffer.clone(),
            index_count: r.mesh.index_count,
        });
    }
}
