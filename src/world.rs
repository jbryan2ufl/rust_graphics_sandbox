use crate::{
    app::State,
    camera::Camera,
    material::{Binding, Material},
    // mesh::create_test_mesh,
    mesh::load_gltf,
    model::Model,
    shader::Shader,
};

use std::sync::Arc;
use std::time::Instant;

pub struct World {
    pub camera: Camera,
    materials: Vec<Arc<Material>>,
    models: Vec<Model>,
    shaders: Vec<Shader>,
    start_time: Instant,
}

impl World {
    pub fn new(state: &State) -> Self {
        let mut bindings = vec![];
        let mut materials = vec![];
        let mut models = vec![];
        let mut shaders = vec![];

        let camera = Camera::new(state);

        bindings.push(Binding {
            buffer: camera.buffer_ref().clone(),
            visibility: wgpu::ShaderStages::VERTEX,
        });
        shaders.push(Shader::new(
            "shaders/model.vert.spv",
            "shaders/model.frag.spv",
        ));
        materials.push(Material::new_arc(state, bindings, shaders.last().unwrap()));

        // let test_mesh = create_test_mesh(&state);
        // models.push(Model {
        //	 mesh: test_mesh,
        //	 material: materials.last().unwrap().clone(),
        // });

        let test_mesh = load_gltf(&state.device, "models/Fox.gltf");
        models.push(Model {
            mesh: test_mesh.last().unwrap().clone(),
            material: materials.last().unwrap().clone(),
        });

        let start_time = Instant::now();

        World {
            camera,
            materials,
            models,
            shaders,
            start_time,
        }
    }

    pub fn render(&self, renderpass: &mut wgpu::RenderPass) {
        for model in &self.models {
            model.render(renderpass);
        }
    }
}
