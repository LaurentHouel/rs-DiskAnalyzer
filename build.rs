use std::collections::BTreeSet;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    generate_dependencies();

    // Embeds the icon shown by Explorer for the .exe (the window icon is set at runtime).
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("failed to embed the Windows icon");
    }
}

fn generate_dependencies() {
    let manifest_path = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo did not provide CARGO_MANIFEST_DIR"),
    )
    .join("Cargo.toml");
    let source = std::fs::read_to_string(manifest_path).expect("failed to read Cargo.toml");
    let manifest: toml::Table = source.parse().expect("failed to parse Cargo.toml");
    let mut dependencies = BTreeSet::new();
    add_dependencies(
        manifest
            .get("dependencies")
            .expect("Cargo.toml is missing [dependencies]"),
        &mut dependencies,
    );
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            if let Some(target_dependencies) = target
                .as_table()
                .and_then(|target| target.get("dependencies"))
            {
                add_dependencies(target_dependencies, &mut dependencies);
            }
        }
    }

    let mut generated = format!("const DEPENDENCIES: [&str; {}] = [\n", dependencies.len());
    for dependency in dependencies {
        generated.push_str(&format!("    {dependency:?},\n"));
    }
    generated.push_str("];\n");
    let output_dir =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR"));
    std::fs::write(output_dir.join("dependencies.rs"), generated)
        .expect("failed to write generated dependency list");
}

fn add_dependencies(value: &toml::Value, dependencies: &mut BTreeSet<String>) {
    let table = value
        .as_table()
        .expect("Cargo dependencies must be represented by a table");
    for (alias, specification) in table {
        let package = specification
            .as_table()
            .and_then(|table| table.get("package"))
            .and_then(toml::Value::as_str)
            .unwrap_or(alias);
        dependencies.insert(package.to_owned());
    }
}
