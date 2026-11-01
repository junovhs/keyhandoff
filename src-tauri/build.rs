fn main() {
    tlshook();
    tauri_build::build()
}

/// On Linux, build the TLS hook `keyhandoff run` preloads into programs
/// (hook/keyhandoff_tls.c) as a shared library; the app embeds it.
fn tlshook() {
    println!("cargo:rerun-if-changed=hook/keyhandoff_tls.c");
    println!("cargo:rerun-if-changed=hook/keyhandoff_node.cjs");
    println!("cargo:rerun-if-changed=hook/hpack_huffman.h");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("libkeyhandoff_tls.so");
    let compiler = cc::Build::new().opt_level(2).get_compiler();
    let status = compiler
        .to_command()
        .args(["-shared", "-fPIC", "-Wall", "-Wextra", "-fvisibility=default", "-o"])
        .arg(&out)
        .arg("hook/keyhandoff_tls.c")
        .args(["-ldl", "-lpthread"])
        .status()
        .expect("a C compiler builds the TLS hook");
    assert!(status.success(), "building hook/keyhandoff_tls.c failed");
}
