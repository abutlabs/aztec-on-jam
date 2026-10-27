// Links Barretenberg's verifier, compiled for rv64e by ../build.sh into ../out/lib/libchonkbb.a.
fn main() {
    if std::env::var("TARGET").map_or(false, |t| t.contains("polkavm")) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../out/lib");
        let dir = dir.canonicalize().expect("../out/lib: run ../build.sh first");
        println!("cargo:rerun-if-changed={}", dir.join("libchonkbb.a").display());
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=static=chonkbb");
    }
}
