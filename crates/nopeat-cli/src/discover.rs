use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub enum Input {
    File(PathBuf),
    Folder(PathBuf),
}

impl Input {
    pub fn label(&self) -> String {
        match self {
            Input::File(p) => file_name(p),
            Input::Folder(p) => p
                .file_name()
                .map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string()),
        }
    }
}

pub fn discover(path: &Path) -> Result<Input> {
    if path.is_file() {
        return Ok(Input::File(path.to_path_buf()));
    }
    if path.is_dir() {
        return Ok(Input::Folder(path.to_path_buf()));
    }
    anyhow::bail!("{} does not exist", path.display())
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string())
}

pub fn read_head(path: &Path, limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut buf = vec![0u8; limit];
    let mut filled = 0;
    while filled < limit {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

pub fn sniff_tool(head: &[u8]) -> &'static str {
    let has = |needle: &[u8]| head.windows(needle.len()).any(|w| w == needle);
    if has(b"\"inputs\"") && has(b"\"outputs\"") && !has(b"\"modules\"") {
        "esbuild"
    } else if has(b"\"isEntry\"") || (has(b"\"file\"") && has(b"\"src\"")) {
        "vite"
    } else if has(b"\"pages\"") || has(b"\"app\"") {
        "next"
    } else {
        "webpack"
    }
}

pub fn tool_for(path: &Path, head: &[u8]) -> &'static str {
    match file_name(path).as_str() {
        "manifest.json" => "vite",
        n if n.ends_with("build-manifest.json") => "next",
        _ => sniff_tool(head),
    }
}

pub fn input_stamp(path: &Path) -> u64 {
    fn mtime(p: &std::fs::Metadata) -> u64 {
        // `as_millis` is a u128. A modification time in milliseconds since the
        // epoch does not come close to the u64 bound, but saturating says so
        // rather than truncating if that ever changes.
        p.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }

    fn dir_stamp(dir: &Path, depth: usize, max: &mut u64) {
        if depth > 8 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.filter_map(std::result::Result::ok) {
            let p = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            let m = mtime(&meta);
            if m > *max {
                *max = m;
            }
            if meta.is_dir() {
                dir_stamp(&p, depth + 1, max);
            }
        }
    }

    let mut stamp = 0u64;
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_dir() => dir_stamp(path, 0, &mut stamp),
        Ok(meta) => stamp = mtime(&meta),
        Err(_) => {}
    }
    stamp
}
