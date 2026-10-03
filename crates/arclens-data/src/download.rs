//! Downloading the RaidTheory dataset as a GitHub tarball.
//!
//! Only the JSON files ARClens reads are extracted (images are fetched lazily
//! by URL instead), keeping the on-disk copy around a few MB.

use crate::Error;
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Tarball of the dataset's default branch.
pub const RAIDTHEORY_ARCHIVE_URL: &str =
    "https://codeload.github.com/RaidTheory/arcraiders-data/tar.gz/refs/heads/main";

/// Paths (relative to the dataset root) that are worth extracting.
const WANTED_PREFIXES: &[&str] = &["items/", "hideout/", "quests/", "map-events/"];
const WANTED_FILES: &[&str] = &["projects.json", "maps.json", "LICENSE"];

/// Downloads the dataset archive and extracts it into `dest`, replacing any
/// previous copy only once extraction has succeeded.
pub async fn download_raidtheory(client: &reqwest::Client, dest: &Path) -> Result<(), Error> {
    let bytes = client
        .get(RAIDTHEORY_ARCHIVE_URL)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let dest = dest.to_owned();
    tokio::task::spawn_blocking(move || extract_into(&bytes[..], &dest))
        .await
        .map_err(|e| Error::Io(std::io::Error::other(e)))?
}

/// Extracts a `.tar.gz` of the dataset into `dest` (atomic directory swap).
pub fn extract_into(archive: impl Read, dest: &Path) -> Result<(), Error> {
    let staging = dest.with_extension("partial");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;

    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let Some(rel) = dataset_relative(&entry.path()?) else {
            continue;
        };
        // Compared with `/` separators on every platform (Windows paths
        // display with `\`).
        let rel_str = rel
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let wanted = WANTED_FILES.contains(&rel_str.as_str())
            || WANTED_PREFIXES.iter().any(|p| rel_str.starts_with(p));
        let json = rel
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if !wanted || !json && rel_str != "LICENSE" {
            continue;
        }
        let out = staging.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&out)?;
    }

    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    std::fs::rename(&staging, dest)?;
    Ok(())
}

/// Strips GitHub's `<repo>-<ref>/` top-level directory and rejects anything
/// that could escape the destination (`..`, absolute paths).
fn dataset_relative(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    components.next()?; // top-level directory
    let mut rel = PathBuf::new();
    for c in components {
        match c {
            Component::Normal(part) => rel.push(part),
            _ => return None,
        }
    }
    (!rel.as_os_str().is_empty()).then_some(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    fn tarball(files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (path, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, body.as_bytes())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn extracts_only_wanted_json_and_strips_top_dir() {
        let archive = tarball(&[
            ("arcraiders-data-main/items/battery.json", "{}"),
            ("arcraiders-data-main/projects.json", "[]"),
            ("arcraiders-data-main/images/battery.png", "png"),
            ("arcraiders-data-main/README.md", "readme"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("raidtheory");
        extract_into(&archive[..], &dest).unwrap();

        assert!(dest.join("items/battery.json").is_file());
        assert!(dest.join("projects.json").is_file());
        assert!(!dest.join("images").exists());
        assert!(!dest.join("README.md").exists());
        assert!(!dir.path().join("raidtheory.partial").exists());
    }

    #[test]
    fn rejects_path_traversal() {
        assert_eq!(dataset_relative(Path::new("top/../etc/passwd")), None);
        assert_eq!(
            dataset_relative(Path::new("top/items/a.json")),
            Some(PathBuf::from("items/a.json"))
        );
    }
}
