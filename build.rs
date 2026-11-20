use std::collections::BTreeSet;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
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

    let lock_path = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo did not provide CARGO_MANIFEST_DIR"),
    )
    .join("Cargo.lock");
    let lock_source = std::fs::read_to_string(lock_path).expect("failed to read Cargo.lock");
    let lock: toml::Table = lock_source.parse().expect("failed to parse Cargo.lock");
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock is missing [[package]] entries");
    let root_package = packages
        .iter()
        .filter_map(toml::Value::as_table)
        .find(|package| {
            package.get("name").and_then(toml::Value::as_str) == Some(env!("CARGO_PKG_NAME"))
                && package.get("version").and_then(toml::Value::as_str)
                    == Some(env!("CARGO_PKG_VERSION"))
        })
        .expect("Cargo.lock is missing the current package");
    let locked_dependencies = root_package
        .get("dependencies")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock is missing the current package dependencies");

    let mut generated = format!(
        "const DEPENDENCIES: [(&str, &str); {}] = [\n",
        dependencies.len()
    );
    for dependency in dependencies {
        let version = locked_version(&dependency, locked_dependencies, packages);
        generated.push_str(&format!("    ({dependency:?}, {version:?}),\n"));
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

fn locked_version(
    dependency: &str,
    locked_dependencies: &[toml::Value],
    packages: &[toml::Value],
) -> String {
    let lock_entry = locked_dependencies
        .iter()
        .filter_map(toml::Value::as_str)
        .find(|entry| lock_entry_name(entry) == dependency)
        .unwrap_or_else(|| panic!("Cargo.lock is missing dependency {dependency}"));

    if let Some((_, version)) = lock_entry.split_once(' ') {
        return version
            .split_whitespace()
            .next()
            .expect("Cargo.lock dependency entry is missing its version")
            .to_owned();
    }

    let versions: BTreeSet<_> = packages
        .iter()
        .filter_map(toml::Value::as_table)
        .filter(|package| package.get("name").and_then(toml::Value::as_str) == Some(dependency))
        .filter_map(|package| package.get("version").and_then(toml::Value::as_str))
        .collect();
    if versions.len() != 1 {
        panic!("Cargo.lock does not identify a unique version for {dependency}");
    }
    versions
        .into_iter()
        .next()
        .expect("dependency version was checked above")
        .to_owned()
}

fn lock_entry_name(entry: &str) -> &str {
    entry.split_once(' ').map_or(entry, |(name, _)| name)
}
