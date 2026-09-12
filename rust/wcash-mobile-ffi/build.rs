fn main() {
    println!("cargo:rerun-if-changed=src/zingo.udl");
    uniffi_build::generate_scaffolding("src/zingo.udl").expect("valid Wcash mobile UDL");
}
