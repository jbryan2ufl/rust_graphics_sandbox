use crate::app::State;
use std::fmt;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub struct Camera {
    uniform: CameraUniform,
    buffer: Arc<wgpu::Buffer>,
    pub eye: glam::Vec3,
    pub center: glam::Vec3,
    pub up: glam::Vec3,
    view: glam::Mat4,
    pub fov: f32,
    aspect_ratio: f32,
    pub z_near: f32,
    pub z_far: f32,
    projection: glam::Mat4,
}

impl Camera {
    pub fn new(state: &State) -> Self {
        let mut uniform = CameraUniform {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        };
        let buffer = Arc::new(
            state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                }),
        );
        let eye = glam::vec3(0.0, 0.0, 5.0);
        let center = glam::Vec3::ZERO;
        let up = glam::Vec3::Y;
        let view = glam::Mat4::look_at_rh(eye, center, up);

        let fov = 70.0_f32.to_radians();
        let aspect_ratio = state.surface_config.width as f32 / state.surface_config.height as f32;
        let z_near = 0.1;
        let z_far = 1000.0;
        let projection = glam::Mat4::perspective_rh_gl(fov, aspect_ratio, z_near, z_far);

        uniform.view_proj = (projection * view).to_cols_array_2d();

        Camera {
            uniform,
            buffer,
            eye,
            center,
            up,
            view,
            fov,
            aspect_ratio,
            z_near,
            z_far,
            projection,
        }
    }

    pub fn buffer_ref(&self) -> &Arc<wgpu::Buffer> {
        &self.buffer
    }

    pub fn update_uniform(&mut self) {
        let view = glam::Mat4::look_at_rh(self.eye, self.center, self.up);
        let projection =
            glam::Mat4::perspective_rh_gl(self.fov, self.aspect_ratio, self.z_near, self.z_far);
        self.uniform.view_proj = (projection * view).to_cols_array_2d();
    }

    pub fn queue_uniform(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}

impl fmt::Debug for Camera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Eye:\n{}\nCenter:\n{}\nUp:\n{}\n\nFOV:\n{}\nZ:\n{:?}\n\nView:\n{}\nProjection:\n{}\n\nUniform:\n{}",
            self.eye,
            self.center,
            self.up,
            self.fov,
            [self.z_near, self.z_far,],
            pretty_mat4(&self.view),
            pretty_mat4(&self.projection),
            pretty_array4x4(&self.uniform.view_proj),
        )
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

fn pretty_mat4(m: &glam::Mat4) -> String {
    let cols = m.to_cols_array_2d();
    let mut s = String::new();
    for row in 0..4 {
        s.push_str("    ");
        s.push_str("[ ");
        for col in 0..4 {
            s.push_str(&format!("{:8.4}", cols[col][row]));
            if col != 3 {
                s.push_str(", ");
            }
        }
        s.push_str(" ]\n");
    }
    s
}

fn pretty_array4x4(m: &[[f32; 4]; 4]) -> String {
    let mut s = String::new();
    for row in 0..4 {
        s.push_str("    [ ");
        for col in 0..4 {
            s.push_str(&format!("{:8.4}", m[col][row]));
            if col != 3 {
                s.push_str(", ");
            }
        }
        s.push_str(" ]\n");
    }
    s
}
