use crate::app::State;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

pub fn create_test_mesh(state: &State) -> Arc<Mesh> {
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

    let vertex_buffer = state
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

    let indices = [0, 1, 2];
    let index_buffer = state
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

    println!("VERTICES: {:?}", &verts[..3]);
    println!("INDICES: {:?}", &indices[..3]);

    Arc::new(Mesh {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    })
}

pub fn load_gltf(state: &State, path: &str) -> Vec<Arc<Mesh>> {
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

            let vertex_buffer =
                state
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
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

            let index_buffer = state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            meshes.push(Arc::new(Mesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
            }));
        }
    }
    meshes
}
