//! Pin the shared parser and reject interpreter features that would embed backend assets.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const REV: &str = "47107f660191de2f0b425569506f4daa4faadac9";
const PACKAGES: &[(&str, &str)] = &[("craft-pdf", "0.1.0"), ("hayro-syntax", "0.7.2"), ("hayro-jbig2", "0.3.0")];
const MANIFESTS: &[&str] = &["crates/pdf/Cargo.toml", "vendor/hayro-syntax/Cargo.toml", "vendor/hayro-jbig2/Cargo.toml"];

/// Edition paths must be one clean checkout at the recorded revision; public builds retain
/// the original Git pin. Packaged edition sources can use that recorded Git revision too.
fn parser_origin(metadata: &Value) -> Result<(String, Option<PathBuf>), String> {
    let public = || (format!("git+https://github.com/storytold/craft-libs?rev={REV}#{REV}"), None);
    let Some(root) = metadata["workspace_root"].as_str().map(Path::new) else { return Ok(public()) };
    let pin = root.join("craft-libs.rev");
    let info = match std::fs::symlink_metadata(&pin) {
        Ok(info) => info,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(public()),
        Err(error) => return Err(format!("craft-libs.rev: {error}")),
    };
    if !info.is_file() || info.len() > 42 {
        return Err("craft-libs.rev: expected a regular file containing one commit id".into());
    }
    let revision = std::fs::read_to_string(&pin).map_err(|error| format!("craft-libs.rev: {error}"))?;
    let revision = revision.trim();
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err("craft-libs.rev: expected one lowercase commit id".into());
    }
    let source = format!("git+https://github.com/storytold/craft-libs?rev={revision}#{revision}");
    let packages = metadata["packages"].as_array().ok_or("cargo metadata has no packages")?;
    let local = PACKAGES.iter().all(|(name, _)| packages.iter().any(|package| package["name"] == *name && package["source"].is_null()));
    if !local {
        return Ok((source, None));
    }
    let owner = root.parent().ok_or("edition workspace has no parent")?.join("craft-libs-common");
    let owner = owner.canonicalize().map_err(|error| format!("shared parser owner: {error}"))?;
    let git = |arguments: &[&str]| -> Result<String, String> {
        let output = Command::new("git").current_dir(&owner).args(arguments).output().map_err(|error| format!("shared parser owner: {error}"))?;
        if !output.status.success() {
            return Err("shared parser paths must be a checked-out repository at craft-libs.rev".into());
        }
        String::from_utf8(output.stdout).map(|value| value.trim().to_owned()).map_err(|_| "shared parser owner returned non-UTF-8 data".into())
    };
    if Path::new(&git(&["rev-parse", "--show-toplevel"])?) != owner || git(&["rev-parse", "HEAD"])? != revision {
        return Err("shared parser owner differs from craft-libs.rev".into());
    }
    if !git(&["status", "--porcelain", "--untracked-files=normal"])?.is_empty() {
        return Err("shared parser owner has uncommitted sources".into());
    }
    Ok((source, Some(owner)))
}

pub fn run(root: &Path) -> Result<(), String> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    validate(&metadata)?;
    println!("PDF backend: pinned parser only; no Hayro interpreter, rasterizer or bundled CMaps");
    Ok(())
}

fn validate(metadata: &Value) -> Result<(), String> {
    let packages = metadata.get("packages").and_then(Value::as_array).ok_or("cargo metadata has no packages")?;
    let nodes = metadata.pointer("/resolve/nodes").and_then(Value::as_array).ok_or("cargo metadata has no resolved nodes")?;
    let (source, owner) = parser_origin(metadata)?;
    for ((name, version), relative) in PACKAGES.iter().zip(MANIFESTS) {
        let matches: Vec<_> = packages.iter().filter(|p| p["name"] == *name).collect();
        if matches.len() != 1 || matches[0]["version"] != *version {
            return Err(format!("{name}: expected exactly {version} from {source}"));
        }
        if let Some(owner) = &owner {
            let actual = matches[0]["manifest_path"].as_str().map(Path::new).and_then(|path| path.canonicalize().ok());
            let expected = owner.join(relative).canonicalize().map_err(|error| format!("{name}: shared manifest: {error}"))?;
            if !matches[0]["source"].is_null() || actual.as_deref() != Some(expected.as_path()) {
                return Err(format!("{name}: expected the pinned shared parser manifest"));
            }
        } else if matches[0]["source"] != source {
            return Err(format!("{name}: expected exactly {version} from {source}"));
        }
    }
    for (name, allowed) in [("craft-pdf", "read"), ("hayro-syntax", "std")] {
        let package = packages.iter().find(|p| p["name"] == name).ok_or_else(|| format!("missing {name}"))?;
        let node = nodes.iter().find(|n| n["id"] == package["id"]).ok_or_else(|| format!("{name}: missing resolved node"))?;
        let features = node["features"].as_array().ok_or_else(|| format!("{name}: missing resolved features"))?;
        if features.len() != 1 || features.first().and_then(Value::as_str) != Some(allowed) {
            return Err(format!("{name}: expected only the {allowed} feature"));
        }
    }
    for package in packages {
        let name = package["name"].as_str().unwrap_or("");
        let active = nodes.iter().any(|node| node["id"] == package["id"]);
        if active && matches!(name, "hayro" | "hayro-interpret" | "hayro-cmap" | "krilla") {
            return Err(format!("{name}: EffectCraft must use craft-pdf with only the read feature"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metadata() -> Value {
        let packages: Vec<_> = PACKAGES.iter().map(|(name, version)| {
            json!({"id": name, "name": name, "version": version, "source": format!("git+https://github.com/storytold/craft-libs?rev={REV}#{REV}")})
        }).collect();
        json!({"packages": packages, "resolve": {"nodes": [
            {"id": "hayro-syntax", "features": ["std"]}, {"id": "craft-pdf", "features": ["read"]}
        ]}})
    }

    #[test]
    fn accepts_the_pinned_parser_only_graph() {
        assert!(validate(&metadata()).is_ok());
    }

    #[test]
    fn rejects_unpinned_duplicate_and_missing_parsers() {
        let mut m = metadata();
        m["packages"][0]["source"] = json!("registry");
        assert!(validate(&m).is_err());
        let mut m = metadata();
        let duplicate = m["packages"][0].clone();
        m["packages"].as_array_mut().unwrap().push(duplicate);
        assert!(validate(&m).is_err());
        m["packages"].as_array_mut().unwrap().retain(|p| p["name"] != "hayro-syntax");
        assert!(validate(&m).is_err());
    }

    #[test]
    fn rejects_shared_reader_source_drift() {
        let mut m = metadata();
        m["packages"][0]["source"] = Value::Null;
        assert!(validate(&m).is_err());
    }

    #[test]
    fn rejects_extra_features_and_missing_active_nodes() {
        for (index, feature) in [(0, "images"), (1, "write"), (1, "raster")] {
            let mut m = metadata();
            m["resolve"]["nodes"][index]["features"].as_array_mut().unwrap().push(json!(feature));
            assert!(validate(&m).is_err());
        }
        let mut m = metadata();
        m["resolve"]["nodes"].as_array_mut().unwrap().pop();
        assert!(validate(&m).is_err());
    }

    #[test]
    fn rejects_backend_asset_features_and_missing_resolution() {
        for name in ["hayro", "hayro-interpret", "hayro-cmap", "krilla"] {
            let mut m = metadata();
            m["packages"].as_array_mut().unwrap().push(json!({"id": name, "name": name}));
            m["resolve"]["nodes"].as_array_mut().unwrap().push(json!({"id": name}));
            assert!(validate(&m).is_err());
        }
        let mut m = metadata();
        m["resolve"] = Value::Null;
        assert!(validate(&m).is_err());
    }

    fn edition_metadata() -> (tempfile::TempDir, Value) {
        let directory = tempfile::tempdir().unwrap();
        let owner = directory.path().join("craft-libs-common");
        let receiver = directory.path().join("effectcraft");
        std::fs::create_dir_all(&receiver).unwrap();
        let mut m = metadata();
        m["workspace_root"] = json!(receiver);
        for (index, relative) in ["crates/pdf", "vendor/hayro-syntax", "vendor/hayro-jbig2"].into_iter().enumerate() {
            let path = owner.join(relative).join("Cargo.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("[package]\nname = {:?}\nversion = {:?}\n", PACKAGES[index].0, PACKAGES[index].1)).unwrap();
            m["packages"][index]["source"] = Value::Null;
            m["packages"][index]["manifest_path"] = json!(path);
        }
        let git = |arguments: &[&str]| {
            let result = Command::new("git")
                .current_dir(&owner)
                .args(arguments)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "Daniel Campos Ramos")
                .env("GIT_AUTHOR_EMAIL", "Capitain_Jack@yahoo.com")
                .env("GIT_COMMITTER_NAME", "Daniel Campos Ramos")
                .env("GIT_COMMITTER_EMAIL", "Capitain_Jack@yahoo.com")
                .output()
                .unwrap();
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            String::from_utf8(result.stdout).unwrap().trim().to_owned()
        };
        git(&["init", "--initial-branch=fixture"]);
        git(&["add", "."]);
        git(&["commit", "-m", "Parser source fixture\n\nAssisted-by: GPT-6 via Codex"]);
        let revision = git(&["rev-parse", "HEAD"]);
        std::fs::write(receiver.join("craft-libs.rev"), revision + "\n").unwrap();
        (directory, m)
    }

    #[test]
    fn accepts_one_clean_pinned_edition_parser_owner() {
        let (_directory, m) = edition_metadata();
        assert!(validate(&m).is_ok());
    }

    #[test]
    fn edition_paths_do_not_bypass_parser_and_feature_limits() {
        let (_directory, m) = edition_metadata();
        let mut changed = m.clone();
        changed["packages"][1]["manifest_path"] = json!("/another-owner/hayro-syntax/Cargo.toml");
        assert!(validate(&changed).is_err());
        let mut changed = m.clone();
        changed["packages"][0]["source"] = json!("registry");
        assert!(validate(&changed).is_err());
        let mut changed = m.clone();
        changed["resolve"]["nodes"][1]["features"] = json!(["read", "raster"]);
        assert!(validate(&changed).is_err());
        let mut changed = m;
        changed["packages"].as_array_mut().unwrap().push(json!({"id": "hayro-cmap", "name": "hayro-cmap"}));
        changed["resolve"]["nodes"].as_array_mut().unwrap().push(json!({"id": "hayro-cmap"}));
        assert!(validate(&changed).is_err());
    }

    #[test]
    fn rejects_edition_owner_revision_drift_and_uncommitted_sources() {
        let (directory, m) = edition_metadata();
        let pin = directory.path().join("effectcraft/craft-libs.rev");
        let original = std::fs::read(&pin).unwrap();
        std::fs::write(&pin, "0000000000000000000000000000000000000000\n").unwrap();
        assert!(validate(&m).is_err());
        std::fs::write(&pin, original).unwrap();
        std::fs::write(directory.path().join("craft-libs-common/vendor/hayro-syntax/Cargo.toml"), "changed source").unwrap();
        assert!(validate(&m).is_err());
    }

    #[test]
    fn packaged_edition_git_sources_keep_the_recorded_revision() {
        let (directory, mut m) = edition_metadata();
        let pin = std::fs::read_to_string(directory.path().join("effectcraft/craft-libs.rev")).unwrap();
        let pin = pin.trim();
        for package in m["packages"].as_array_mut().unwrap() {
            package["source"] = json!(format!("git+https://github.com/storytold/craft-libs?rev={pin}#{pin}"));
        }
        assert!(validate(&m).is_ok());
        m["packages"][1]["source"] = json!(format!("git+https://github.com/storytold/craft-libs?rev={REV}#{REV}"));
        assert!(validate(&m).is_err());
    }

    #[test]
    fn rejects_malformed_and_oversized_edition_revision_files() {
        let (directory, m) = edition_metadata();
        let pin = directory.path().join("effectcraft/craft-libs.rev");
        for content in ["not a commit".to_owned(), "f".repeat(1000)] {
            std::fs::write(&pin, content).unwrap();
            assert!(validate(&m).is_err());
        }
    }
}
