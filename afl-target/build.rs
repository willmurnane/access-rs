use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(nightly)");
    let rustc = std::env::var_os("RUSTC").unwrap();
    let output = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .unwrap();

    let out_buf = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_buf);
    let access_dir = out_dir.join("access");
    println!("Cloning into {}", access_dir.to_str().unwrap());
    let clone_output = std::process::Command::new("git")
        .arg("clone")
        .arg("https://github.com/apache/accumulo-access.git")
        .arg(access_dir.to_str().unwrap())
        .output()
        .unwrap();
    println!("{}", String::from_utf8(clone_output.stdout).unwrap());

    println!("Mvn package in {}", access_dir.to_str().unwrap());
    std::process::Command::new("mvn")
        .arg("package")
        .arg("-DskipTests")
        .current_dir(&access_dir)
        .output()
        .unwrap();

    println!(
        "cargo::rustc-env=JAR_LOCATION={}",
        access_dir
            .join("modules/core/target/accumulo-access-core-1.0.0-beta2-SNAPSHOT.jar")
            .to_str()
            .expect("jar location")
    );

    let version_str = String::from_utf8(output.stdout).unwrap();
    if version_str.contains("nightly") {
        println!("cargo:rustc-cfg=nightly");
    }
}
