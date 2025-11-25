fn main() {
    println!("cargo::rustc-check-cfg=cfg(nightly)");
    let rustc = std::env::var_os("RUSTC").unwrap();
    let output = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .unwrap();

    let version_str = String::from_utf8(output.stdout).unwrap();
    if version_str.contains("nightly") {
        println!("cargo:rustc-cfg=nightly");
    }
}
