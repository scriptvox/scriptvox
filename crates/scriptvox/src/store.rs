//! Content-addressed cache (PLAN-R1 §2.7). `cache/blobs/<b3[0:2]>/<b3>` holds
//! the bytes, `cache/keys/<key>.json` maps a cache key to named blobs plus the
//! tool, its version and the output terms. The file path is the index.
//!
//! Verify on read: a blob whose blake3 no longer matches is moved to
//! `cache/quarantine/` and the read is a miss (the MPT #1740 bug class:
//! a corrupt entry reused forever). Writes go to `.part` and are renamed.

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub struct Cas {
    pub root: PathBuf,
}

/// Cache key: blake3 over tool, version and the canonical inputs. serde_json
/// maps are sorted, so key order in `inputs` never changes the key.
pub fn key(tool: &str, version: &str, inputs: &Value) -> String {
    let mut h = blake3::Hasher::new();
    for part in [tool, version, &inputs.to_string()] {
        h.update(part.as_bytes());
        h.update(&[0]);
    }
    h.finalize().to_hex().to_string()
}

pub fn hash_file(p: &Path) -> std::io::Result<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(p)?)?;
    Ok(h.finalize().to_hex().to_string())
}

impl Cas {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn open() -> Self {
        Self::new(crate::provision::data_dir().join("cache"))
    }

    fn blob(&self, h: &str) -> PathBuf {
        self.root.join("blobs").join(&h[..2]).join(h)
    }

    fn entry(&self, key: &str) -> PathBuf {
        self.root.join("keys").join(format!("{key}.json"))
    }

    /// Writes `body` to `dest` through `dest.part` and a rename.
    fn write_atomic(
        dest: &Path,
        write: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(dest.parent().unwrap())?;
        let part = dest.with_extension("part");
        write(&part)?;
        std::fs::rename(&part, dest)
    }

    /// Stores named files under `key`. `terms` are the provider output terms.
    pub fn put(
        &self,
        key: &str,
        tool: &str,
        version: &str,
        files: &[(&str, &Path)],
        terms: Value,
    ) -> std::io::Result<()> {
        let mut blobs = Map::new();
        for (name, path) in files {
            let h = hash_file(path)?;
            let dest = self.blob(&h);
            if !dest.is_file() {
                Self::write_atomic(&dest, |p| std::fs::copy(path, p).map(|_| ()))?;
            }
            blobs.insert(name.to_string(), json!(h));
        }
        let doc = json!({"tool": tool, "version": version, "blobs": blobs, "output_terms": terms});
        Self::write_atomic(&self.entry(key), |p| std::fs::write(p, doc.to_string()))
    }

    /// True when `key` has an entry.
    pub fn has(&self, key: &str) -> bool {
        self.entry(key).is_file()
    }

    /// The terms of an entry, read without copying a file: a small fact stored under a key with no files.
    pub fn note(&self, key: &str) -> Option<Value> {
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(self.entry(key)).ok()?).ok()?;
        Some(doc["output_terms"].clone())
    }

    /// Copies the files of `key` into `dest_dir` under their names. Every blob
    /// is verified first; any mismatch quarantines it and gives a miss. A blob
    /// that is gone is a miss too, and its entry goes, so `has` stops saying yes.
    pub fn get(&self, key: &str, dest_dir: &Path) -> Option<Vec<(String, PathBuf)>> {
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(self.entry(key)).ok()?).ok()?;
        let blobs = doc["blobs"].as_object()?;
        for h in blobs.values().filter_map(Value::as_str) {
            let p = self.blob(h);
            if hash_file(&p).ok().as_deref() != Some(h) {
                let q = self.root.join("quarantine");
                let _ = std::fs::create_dir_all(&q);
                let _ = std::fs::rename(&p, q.join(h));
                let _ = std::fs::remove_file(self.entry(key));
                return None;
            }
        }
        std::fs::create_dir_all(dest_dir).ok()?;
        blobs
            .iter()
            .map(|(name, h)| {
                let to = dest_dir.join(name);
                std::fs::copy(self.blob(h.as_str()?), &to).ok()?;
                Some((name.clone(), to))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Cas, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let cas = Cas::new(d.path().join("cache"));
        let f = d.path().join("voice.mp3");
        std::fs::write(&f, b"some audio bytes").unwrap();
        (d, cas, f)
    }

    #[test]
    fn round_trip() {
        let (d, cas, f) = setup();
        cas.put(
            "k1",
            "tts.edge",
            "0.0.1",
            &[("voice.mp3", &f)],
            json!({"redistribution": false}),
        )
        .unwrap();
        let got = cas.get("k1", &d.path().join("out")).unwrap();
        assert_eq!(std::fs::read(&got[0].1).unwrap(), b"some audio bytes");
    }

    #[test]
    fn flipped_byte_is_a_miss_and_quarantined() {
        let (d, cas, f) = setup();
        cas.put("k1", "tts.edge", "0.0.1", &[("voice.mp3", &f)], json!({}))
            .unwrap();
        let h = hash_file(&f).unwrap();
        let blob = cas.blob(&h);
        let mut bytes = std::fs::read(&blob).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&blob, bytes).unwrap();
        assert!(cas.get("k1", &d.path().join("out")).is_none());
        assert!(cas.root.join("quarantine").join(&h).is_file());
        assert!(!blob.exists());
    }

    #[test]
    fn a_note_has_no_file_and_a_lost_blob_drops_its_entry() {
        let (d, cas, f) = setup();
        cas.put("n", "voice.fit", "1", &[], json!({"rate": 11}))
            .unwrap();
        assert_eq!(cas.note("n").unwrap()["rate"], 11);
        assert!(cas.note("none").is_none());
        cas.put("k1", "t", "v", &[("a", &f)], json!({})).unwrap();
        std::fs::remove_dir_all(cas.root.join("blobs")).unwrap();
        assert!(cas.has("k1"));
        assert!(cas.get("k1", &d.path().join("out")).is_none());
        assert!(!cas.has("k1"), "the plan must not see it as made");
    }

    #[test]
    fn no_part_files_left() {
        let (_d, cas, f) = setup();
        cas.put("k1", "t", "v", &[("a", &f)], json!({})).unwrap();
        let parts = walk(&cas.root)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "part"))
            .count();
        assert_eq!(parts, 0);
    }

    fn walk(p: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .flat_map(|p| if p.is_dir() { walk(&p) } else { vec![p] })
            .collect()
    }
}
