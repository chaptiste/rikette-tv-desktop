fn main() {
    println!("cargo:rerun-if-env-changed=RIKETTE_UPDATER_PUBKEY");
    tauri_build::build();
}
