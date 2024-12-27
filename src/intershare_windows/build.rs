#[cfg(target_os = "windows")]
fn main() {
    uniffi::generate_scaffolding("./src/intershare_sdk.udl").unwrap();
}
#[cfg(not(target_os = "windows"))]
fn main() {
    
}