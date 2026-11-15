//! Locate already-built Rust artifacts without depending on a Cargo test-binary layout.
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Artifacts {
    pub library: PathBuf,
    pub dependencies: Vec<PathBuf>,
    pub profile: PathBuf,
}
pub fn rmagna() -> Result<Artifacts, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    // Cargo can place executables in deps/ or in build/<package>/<hash>/out/.
    let profile = executable
        .ancestors()
        .find_map(|p| {
            matches!(
                p.file_name().and_then(|n| n.to_str()),
                Some("deps" | "build")
            )
            .then(|| p.parent().map(Path::to_path_buf))
            .flatten()
        })
        .ok_or_else(|| {
            format!(
                "cannot identify Cargo profile from test executable {}",
                executable.display()
            )
        })?;
    let mut dependencies = Vec::new();
    let deps = profile.join("deps");
    if deps.is_dir() {
        dependencies.push(deps);
    }
    // Libraries can also live in Cargo's per-target build outputs. Read only
    // this profile's shallow artifact tree; never search the checkout or rebuild.
    let mut pending = vec![(profile.join("build"), 0)];
    while let Some((directory, depth)) = pending.pop() {
        if !directory.is_dir() {
            continue;
        }
        if directory.file_name().is_some_and(|n| n == "out") {
            dependencies.push(directory);
            continue;
        }
        if depth >= 3 {
            continue;
        }
        for entry in
            fs::read_dir(&directory).map_err(|e| format!("{}: {e}", directory.display()))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    dependencies.sort();
    dependencies.dedup();
    let mut libraries = Vec::new();
    for directory in &dependencies {
        for entry in fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.file_name().and_then(|p| p.to_str()).is_some_and(|n| {
                n == "librmagna.rlib" || (n.starts_with("librmagna-") && n.ends_with(".rlib"))
            }) {
                let modified = path
                    .metadata()
                    .and_then(|m| m.modified())
                    .map_err(|e| e.to_string())?;
                libraries.push((modified, path));
            }
        }
    }
    libraries.sort();
    let library = libraries.pop().map(|(_, p)| p).ok_or_else(|| {
        format!(
            "no already-built librmagna rlib in Cargo profile {}; dependencies={dependencies:?}",
            profile.display()
        )
    })?;
    Ok(Artifacts {
        library,
        dependencies,
        profile,
    })
}
