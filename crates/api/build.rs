fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
    println!("cargo:rerun-if-env-changed=PASSWORD_VAULT_BUILD_REVISION");
}
