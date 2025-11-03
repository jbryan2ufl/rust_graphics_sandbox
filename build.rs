use std::process::Command;
fn main() {
    let src = "shaders/triangle.slang";
    Command::new("slangc")
        .args([
            src,
            "-target",
            "spirv",
            "-o",
            "shaders/triangle.vert.spv",
            "-entry",
            "vsMain",
            "-stage",
            "vertex",
            "-fvk-use-entrypoint-name",
        ])
        .status()
        .unwrap();
    Command::new("slangc")
        .args([
            src,
            "-target",
            "spirv",
            "-o",
            "shaders/triangle.frag.spv",
            "-entry",
            "psMain",
            "-stage",
            "pixel",
            "-fvk-use-entrypoint-name",
        ])
        .status()
        .unwrap();

    let src = "shaders/model.slang";
    Command::new("slangc")
        .args([
            src,
            "-target",
            "spirv",
            "-o",
            "shaders/model.vert.spv",
            "-entry",
            "vsMain",
            "-stage",
            "vertex",
            "-fvk-use-entrypoint-name",
        ])
        .status()
        .unwrap();
    Command::new("slangc")
        .args([
            src,
            "-target",
            "spirv",
            "-o",
            "shaders/model.frag.spv",
            "-entry",
            "psMain",
            "-stage",
            "pixel",
            "-fvk-use-entrypoint-name",
        ])
        .status()
        .unwrap();

    println!("cargo:rerun-if-changed={src}");
}
