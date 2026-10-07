fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engines/ghostty");
    cc::Build::new().include(root.join("include")).file("src/shim.c").compile("current_ghostty_shim");
    println!("cargo:rerun-if-changed=src/shim.c");
}
