use anyhow::{Context, Result};

const HTML: &str = include_str!("../../../assets/report/shell.html");
const CSS: &str = include_str!("../../../assets/report/shell.css");
const JS: &str = include_str!("../../../assets/report/shell.js");

pub const INLINE_LIMIT: usize = 2 * 1024 * 1024;

pub const DATA_SCRIPT_PLACEHOLDER: &str = "REPLACED_BY_DATA_SCRIPT";

pub fn render(
    payload: &serde_json::Value,
    _target: &str,
    title: &str,
    detail: &[u8],
    inline_detail: bool,
) -> Result<String> {
    let json = escape_for_script_tag(&serde_json::to_string(payload)?);

    let detail_block = if inline_detail {
        format!(
            "<script id=\"detail\" type=\"application/json\">{}</script>",
            escape_for_script_tag(std::str::from_utf8(detail).unwrap_or("{}"))
        )
    } else {
        format!("<script src=\"{DATA_SCRIPT_PLACEHOLDER}\"></script>")
    };

    Ok(HTML
        .replace("/*TITLE*/", &escape_html(title))
        .replace("/*CSS*/", CSS)
        .replace("/*PAYLOAD*/", &json)
        .replace("<!--DETAIL-->", &detail_block)
        .replace("/*JS*/", JS))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn write_detail_file(
    graph: &nopeat_core::model::UnifiedBundleGraph,
    out: &std::path::Path,
) -> Result<u64> {
    use std::io::Write;
    let data_path = data_path(out);
    let file = std::fs::File::create(&data_path)
        .with_context(|| format!("creating {}", data_path.display()))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, file);
    w.write_all(b"window.__OBS_DETAIL__=")?;
    nopeat_core::report::write_detail(graph, &mut w)
        .with_context(|| format!("writing {}", data_path.display()))?;
    w.write_all(b";\n")?;
    w.flush()?;
    Ok(std::fs::metadata(&data_path)?.len())
}

pub fn write(
    payload: &serde_json::Value,
    target: &str,
    title: &str,
    out: &std::path::Path,
    detail_bytes: u64,
) -> Result<()> {
    let data_path = data_path(out);
    let inline_detail = detail_bytes <= INLINE_LIMIT as u64;
    let html = if inline_detail {
        let detail = std::fs::read(&data_path)
            .with_context(|| format!("reading back {}", data_path.display()))?;
        render(payload, target, title, &detail, true)?
    } else {
        let file_name = data_path
            .file_name()
            .map_or_else(|| "report.data.js".to_string(), |n| n.to_string_lossy().to_string());

        let mut html = render(payload, target, title, &[], false)?;
        html = html.replace(DATA_SCRIPT_PLACEHOLDER, &file_name);
        html
    };

    std::fs::write(out, html).with_context(|| format!("writing {}", out.display()))?;
    if inline_detail {
        let _ = std::fs::remove_file(&data_path);
    }
    Ok(())
}

pub fn data_path(report: &std::path::Path) -> std::path::PathBuf {
    let mut name = report.file_name().unwrap_or_default().to_os_string();
    name.push(".data.js");
    report.with_file_name(name)
}

fn escape_for_script_tag(s: &str) -> String {
    s.replace("</", "<\\/")
}
