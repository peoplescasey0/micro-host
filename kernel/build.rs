fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}", dir);
    println!("cargo:rerun-if-changed=linker.ld");
}
