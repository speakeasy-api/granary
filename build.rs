use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    assert_unique_migration_versions();
}

fn assert_unique_migration_versions() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));
    let migrations_dir = manifest_dir.join("migrations");

    let entries = fs::read_dir(&migrations_dir).unwrap_or_else(|error| {
        panic!(
            "failed to read migrations directory '{}': {error}",
            migrations_dir.display()
        )
    });

    let mut versions_to_files: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for entry in entries {
        let entry = entry.expect("failed to read a migration directory entry");
        let file_type = entry
            .file_type()
            .expect("failed to read migration file type");
        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.ends_with(".sql") {
            continue;
        }

        let (version, _) = file_name.split_once('_').unwrap_or_else(|| {
            panic!(
                "invalid migration filename '{}': expected '<version>_<name>.sql'",
                file_name
            )
        });

        if version.is_empty() || !version.chars().all(|ch| ch.is_ascii_digit()) {
            panic!(
                "invalid migration version '{}' in '{}': expected numeric prefix",
                version, file_name
            );
        }

        versions_to_files
            .entry(version.to_string())
            .or_default()
            .push(file_name.to_string());
    }

    let duplicates: Vec<(String, Vec<String>)> = versions_to_files
        .into_iter()
        .filter_map(|(version, mut files)| {
            if files.len() > 1 {
                files.sort();
                Some((version, files))
            } else {
                None
            }
        })
        .collect();

    if duplicates.is_empty() {
        return;
    }

    let mut details = String::new();
    for (version, files) in duplicates {
        details.push_str(&format!("  {version}: {}\n", files.join(", ")));
    }

    panic!(
        "duplicate SQL migration versions detected in 'migrations/'\n\
         recreate one migration with a new version (use `sqlx migrate add <name>`)\n\
         conflicting versions:\n{details}"
    );
}
