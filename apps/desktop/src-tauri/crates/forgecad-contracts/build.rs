use std::env;

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn main() {
    println!("cargo:rerun-if-env-changed=FORGECAD_BUILD_COHORT_SHA256");
    if let Ok(value) = env::var("FORGECAD_BUILD_COHORT_SHA256") {
        if is_sha256(&value) {
            println!("cargo:rustc-env=FORGECAD_BUILD_COHORT_SHA256={value}");
        }
    }
}
