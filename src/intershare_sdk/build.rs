fn main() {
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=13.0");
    uniffi::generate_scaffolding("./src/intershare_sdk.udl").unwrap();
}
