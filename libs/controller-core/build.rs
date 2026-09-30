use sha2::{Digest, Sha256};

fn source(path: &str, expected: &str) -> String {
    println!("cargo:rerun-if-changed={path}");
    let content = std::fs::read_to_string(path)
        .expect("upstream source")
        .replace("\r\n", "\n");
    let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    assert_eq!(
        hash, expected,
        "Upstream security source changed: review extraction and vectors before updating its hash"
    );
    content
}

fn between<'a>(content: &'a str, start: &str, end: &str) -> &'a str {
    let start = content.find(start).expect("upstream start anchor");
    let end = content[start..].find(end).expect("upstream end anchor") + start;
    &content[start..end]
}

fn main() {
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let proto = "../base/protos/message.proto";
    let rendezvous = "../hbb_common/protos/rendezvous.proto";
    println!("cargo:rerun-if-changed={proto}");
    println!("cargo:rerun-if-changed={rendezvous}");
    let proto_out = format!("{out}/protos");
    std::fs::create_dir_all(&proto_out).expect("protobuf output directory");
    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir(&proto_out)
        .inputs([proto, rendezvous])
        .include("../base/protos")
        .include("../hbb_common/protos")
        .run()
        .expect("protobuf codegen");

    let tcp = source(
        "../hbb_common/src/tcp.rs",
        "f354dc407bf30ff13f8289c4efe1e3f031333748e0da67ec401830f74285debc",
    );
    let common = source(
        "../../src/common.rs",
        "d9bc63c5c504213c9413a7fe9fa07405273f44ecdb2f374aa52f07d818c6a21d",
    );
    let declarations = between(
        &tcp,
        "pub const KX_VERSION_LATEST",
        "pub struct FramedStream(",
    );
    let cipher = tcp[tcp.find("impl Encrypt {").expect("Encrypt implementation")..]
        .lines()
        .map(|line| {
            if line.trim().starts_with("fn test_") && line.trim().ends_with("() {") {
                format!(
                    "{line}\n        sodiumoxide::init().expect(\"crypto test initialization\");"
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let nonce = between(&tcp, "    fn get_nonce(", "\n}\n\nconst DEFAULT_BACKLOG");
    let pk = between(&common, "fn get_pk(", "\n#[inline]\npub fn get_rs_pk");
    let identity = between(
        &common,
        "pub fn decode_id_pk(",
        "\n#[inline]\npub fn using_public_server",
    );
    let generated = format!("{declarations}\nstruct FramedStream;\nimpl FramedStream {{\n{nonce}\n}}\n{cipher}\n{pk}\n{identity}");
    std::fs::write(format!("{out}/upstream_crypto.rs"), generated)
        .expect("write upstream crypto module");
}
