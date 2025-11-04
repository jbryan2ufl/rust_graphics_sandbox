pub struct Shader {
    pub vertex_binary: Vec<u8>,
    pub pixel_binary: Vec<u8>,
}

impl Shader {
    pub fn new(vertex_path: &str, pixel_path: &str) -> Self {
        let vertex_binary = std::fs::read(vertex_path).unwrap();
        let pixel_binary = std::fs::read(pixel_path).unwrap();
        Shader {
            vertex_binary,
            pixel_binary,
        }
    }
}
