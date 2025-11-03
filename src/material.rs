use crate::app::State;
use std::sync::Arc;

use crate::shader::Shader;

pub struct Binding {
    pub buffer: Arc<wgpu::Buffer>,
    pub visibility: wgpu::ShaderStages,
}

pub struct Material {
    bind_group_layouts: Vec<wgpu::BindGroupLayout>,
    pub bind_groups: Vec<wgpu::BindGroup>,
    pipeline_layout: wgpu::PipelineLayout,
    pub pipeline: Arc<wgpu::RenderPipeline>,
}

impl Material {
    pub fn new_arc(state: &State, bindings: Vec<Binding>, shader: &Shader) -> Arc<Self> {
        let mut bind_groups = vec![];
        let mut bind_group_layouts = vec![];
        for binding in bindings {
            bind_group_layouts.push(state.device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: binding.visibility,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                },
            ));
            bind_groups.push(state.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: bind_group_layouts.last().unwrap(),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: binding.buffer.as_entire_binding(),
                }],
                label: None,
            }));
        }

        let swapchain_capabilities = state.surface.get_capabilities(&state.adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let pipeline_layout =
            state
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &bind_group_layouts.iter().collect::<Vec<_>>(),
                    push_constant_ranges: &[],
                });
        let pipeline = Arc::new(
            state
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &state
                            .device
                            .create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: None,
                                source: wgpu::ShaderSource::SpirV(
                                    bytemuck::cast_slice(&shader.vertex_binary).into(),
                                ),
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
                        module: &state
                            .device
                            .create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: None,
                                source: wgpu::ShaderSource::SpirV(
                                    bytemuck::cast_slice(&shader.pixel_binary).into(),
                                ),
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
                }),
        );

        Arc::new(Material {
            bind_group_layouts,
            bind_groups,
            pipeline_layout,
            pipeline,
        })
    }
}
