// The service's configuration is compiled in: cfg.bin beside this file if present (measure.sh
// or a deployment writes it; jam080-build clears the environment, so it can't be an env var),
// else cfg.default.bin (a zero genesis, for builds that only check the code compiles).
fn main() {
    println!("cargo:rerun-if-changed=cfg.bin");
    println!("cargo:rerun-if-changed=cfg.default.bin");
    let src = if std::path::Path::new("cfg.bin").exists() { "cfg.bin" } else { "cfg.default.bin" };
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("cfg.bin");
    std::fs::copy(src, out).expect("the cfg file");
}
