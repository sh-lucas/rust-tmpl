fn main() {
    let changelog = std::fs::read_to_string("specs/changelog.md").expect("read changelog");
    let version = changelog
        .lines()
        .find_map(|line| line.strip_prefix("## v"))
        .filter(|version| {
            let mut parts = version.split('.');
            (0..3).all(|_| {
                parts.next().is_some_and(|part| {
                    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                })
            }) && parts.next().is_none()
        })
        .expect("changelog must start with a ## vX.Y.Z version");
    println!("cargo:rustc-env=APP_VERSION={version}");
    println!("cargo:rerun-if-changed=specs/changelog.md");
}
