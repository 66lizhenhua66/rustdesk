fn main() {
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let proto = "../base/protos/message.proto";
    println!("cargo:rerun-if-changed={proto}");
    let out = format!("{out}/protos");
    std::fs::create_dir_all(&out).expect("protobuf output directory");
    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir(&out)
        .inputs([proto])
        .include("../base/protos")
        .run()
        .expect("protobuf codegen");
}
