#[cfg(target_os = "windows")]
fn main() {
    uniffi::uniffi_bindgen_main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    
}