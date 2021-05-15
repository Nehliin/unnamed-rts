use anyhow::*;
use glob::glob;
use naga::{
    back::spv::{self, WriterFlags},
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};
use rayon::prelude::*;
use std::fs::{read_to_string, write};
use std::path::PathBuf;

struct ShaderData {
    src: String,
    spv_path: PathBuf,
}

impl ShaderData {
    pub fn load(src_path: PathBuf) -> Result<Self> {
        let src = read_to_string(src_path.clone())?;
        let spv_path = src_path.with_extension("spv");

        Ok(ShaderData {
            src,
            spv_path,
        })
    }
}

fn main() -> Result<()> {
    let mut shader_paths = Vec::new();
    shader_paths.extend(glob("./src/graphics/shaders/**/*.wgsl")?);

    let shaders = shader_paths
        .into_par_iter()
        .map(|glob_result| ShaderData::load(glob_result?))
        .collect::<Vec<Result<_>>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    for shader in shaders {
        let module = match wgsl::parse_str(&shader.src) {
            Ok(module) => module, 
            Err(err) => panic!("Failed to parse shader: {}", err.emit_to_string(&shader.src))
        };
        let info = Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module)?;
        let mut flags = WriterFlags::empty();
        // This matches what's currently used in wgpu core
        flags.set(WriterFlags::DEBUG, cfg!(debug_assertions));
        let options = spv::Options {
            lang_version: (1, 0),
            flags,
            capabilities: Some(
                [
                    spv::Capability::Shader,
                    spv::Capability::DerivativeControl,
                    spv::Capability::InterpolationFunction,
                    spv::Capability::Matrix,
                    spv::Capability::ImageQuery,
                    spv::Capability::Sampled1D,
                    spv::Capability::Image1D,
                    spv::Capability::SampledCubeArray,
                    spv::Capability::ImageCubeArray,
                    spv::Capability::ImageMSArray,
                    spv::Capability::StorageImageExtendedFormats,
                ]
                .iter()
                .cloned()
                .collect(),
            ),
        };
        let compiled = spv::write_vec(&module, &info, &options)?;
        let binary = unsafe {
            std::slice::from_raw_parts(compiled.as_ptr() as *const u8, compiled.len() * 4)
        };
        write(shader.spv_path, &binary)?;
    }
    println!("cargo:rerun-if-changed=./src/bin/client/graphics/shaders/");

    Ok(())
}
