use std::env;
use std::io::Write;
use std::path::PathBuf;
use prost_wkt_build::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    // Print the current working directory
    println!("Current working directory: {:?}", std::env::current_dir()?);
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let descriptor_file = out.join("descriptors.bin");
    let mut prost_build = prost_build::Config::new();
    println!("Descriptor file: {:?}", descriptor_file);
    prost_build
        .type_attribute(
            ".",
            "#[derive(serde::Serialize,serde::Deserialize)]"
        )
        .extern_path(
            ".google.protobuf.Any",
            "::prost_wkt_types::Any"
        )
        .extern_path(
            ".google.protobuf.Timestamp",
            "::prost_wkt_types::Timestamp"
        )
        .extern_path(
            ".google.protobuf.Value",
            "::prost_wkt_types::Value"
        )
        .file_descriptor_set_path(&descriptor_file)
        .compile_protos(
            &["src/submodules/message-protos/commands.proto",
                     "src/submodules/message-protos/models.proto",
                     "src/submodules/message-protos/events.proto"],
            &["src/submodules/message-protos"],
        )
        .unwrap();

    let descriptor_bytes =
        std::fs::read(descriptor_file)
            .unwrap();
    println!("Descriptor bytes: {:?}", descriptor_bytes.len());

    let descriptor =
        FileDescriptorSet::decode(&descriptor_bytes[..])
            .unwrap();

    prost_wkt_build::add_serde(out.clone(), descriptor);

    // Create a infra_old file in the output directory
    let mut mod_file = std::fs::File::create(out.join("mod.rs"))?;
    mod_file.write_all(b"pub mod commands;\n")?;
    mod_file.write_all(b"pub mod models;\n")?;
    mod_file.write_all(b"pub mod events;\n")?;


    Ok(())
}
