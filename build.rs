fn main() {
    // Cargo does not track the linker script passed via rustflags; relink when it changes.
    println!("cargo:rerun-if-changed=linker.ld");
}
