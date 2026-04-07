fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../proto");
    println!("cargo:rerun-if-changed=build.rs");

    tonic_prost_build::configure()
        .compile_protos(&["../../proto/identity.proto"], &["../../proto"])?;

    Ok(())
}
