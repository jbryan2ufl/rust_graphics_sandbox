use crate::material::Material;
use crate::mesh::Mesh;
use std::sync::Arc;

pub struct Model {
    pub mesh: Arc<Mesh>,
    pub material: Arc<Material>,
}

impl Model {
    pub fn render(&self, renderpass: &mut wgpu::RenderPass) {
        renderpass.set_pipeline(&self.material.pipeline);
        renderpass.set_bind_group(0, &self.material.bind_groups[0], &[]);
        renderpass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
        renderpass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        renderpass.draw_indexed(0..self.mesh.index_count, 0, 0..1);
    }
}
