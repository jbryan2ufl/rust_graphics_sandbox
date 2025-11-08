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
}

pub fn load_gltf(device: &wgpu::Device, path: &str) -> Vec<Arc<Mesh>> {
    let (doc, buffs, _) = gltf::import(path).unwrap();
    let mut meshes = vec![];

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| Some(&buffs[b.index()]));

            let Some(pos_iter) = reader.read_positions() else {
                return vec![];
            };
            let positions: Vec<[f32; 3]> = pos_iter.collect();
            if positions.is_empty() {
                return vec![];
            }

            let vertex_count = positions.len();

            let mut verts = Vec::<Vertex>::with_capacity(positions.len());
            (0..vertex_count).for_each(|i| verts.push(Vertex { pos: positions[i] }));

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

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

            meshes.push(Arc::new(Mesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
            }));
        }
    }
    meshes
}
