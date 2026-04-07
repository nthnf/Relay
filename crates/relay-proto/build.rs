fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../proto");
    println!("cargo:rerun-if-changed=build.rs");

    tonic_prost_build::configure().compile_protos(
        &[
            "../../proto/bootstrap.proto",
            "../../proto/chat.proto",
            "../../proto/friendship.proto",
            "../../proto/identity.proto",
            "../../proto/realtime.proto",
            "../../proto/workspace.proto",
        ],
        &["../../proto"],
    )?;

    Ok(())
}
