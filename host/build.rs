fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe { std::env::set_var("PROTOC", protoc) };
    prost_build::compile_protos(&["../proto/decky_my_rig.proto"], &["../proto"])?;
    println!("cargo:rerun-if-changed=../proto/decky_my_rig.proto");
    Ok(())
}
