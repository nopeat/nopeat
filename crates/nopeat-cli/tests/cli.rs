use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let exe = if cfg!(windows) { "nopeat.exe" } else { "nopeat" };
    let candidate = path.join(exe);
    assert!(candidate.exists(), "the CLI binary is missing at {}", candidate.display());
    candidate
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nopeat-cli-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn vlq(value: i64) -> String {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut v = if value < 0 { ((-value) << 1) | 1 } else { value << 1 };
    let mut out = String::new();
    loop {
        let mut digit = usize::try_from(v & 31).unwrap_or(0);
        v >>= 5;
        if v > 0 {
            digit |= 32;
        }
        out.push(B64[digit] as char);
        if v == 0 {
            return out;
        }
    }
}

fn two_source_map(split_at: u32) -> String {
    let first = format!("{}{}{}{}", vlq(0), vlq(0), vlq(0), vlq(0));
    let second = format!("{}{}{}{}", vlq(i64::from(split_at)), vlq(1), vlq(0), vlq(0));
    format!(
        r#"{{"version":3,"file":"index.js","sources":["webpack:///src/first.js","webpack:///src/second.js"],"sourcesContent":[],"names":[],"mappings":"{first},{second}"}}"#
    )
}
fn run(dir: &Path) -> (String, String, i32) {
    let out = Command::new(binary())
        .arg(dir)
        .args(["--mode", "static", "--report"])
        .arg(dir.join("report.html"))
        .output()
        .expect("the CLI runs");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn a_vite_dist_folder_without_bundler_metadata_is_analysed() {
    let dir = temp_dir("no-metadata");

    fs::create_dir_all(dir.join("assets")).expect("create assets dir");
    fs::write(dir.join("index.html"), b"<!doctype html>").expect("write html");
    let map = two_source_map(2_048);
    fs::write(dir.join("assets/index-DiwrgTda.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("assets/index-DiwrgTda.js.map"), map.as_bytes()).expect("write map");

    let (stdout, stderr, code) = run(&dir);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("2 assets"), "the nested bundle and the html both count: {stdout}");
    assert!(stdout.contains("modules attributed"), "no fusion line in: {stdout}");
    assert!(
        stdout.contains("2/2 modules attributed"),
        "both sources should be attributed from the map alone: {stdout}"
    );
    assert!(dir.join("report.html").exists(), "no report was written");
}

#[test]
fn no_declared_graph_means_no_ghost_claim() {
    let dir = temp_dir("ghost-honesty");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");

    let (stdout, _, _) = run(&dir);
    assert!(
        stdout.contains("ghost code needs a stats.json to detect"),
        "a folder with no declared graph must not report a ghost count: {stdout}"
    );
    assert!(
        !stdout.contains("0 ghost"),
        "'0 ghost' would be an absence of evidence dressed as a clean bill of health: {stdout}"
    );
}

#[test]
fn the_report_is_not_counted_as_one_of_the_assets() {
    let dir = temp_dir("report-not-an-asset");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");

    let (first, _, first_code) = run(&dir);
    assert_eq!(first_code, 0, "{first}");
    let (second, _, second_code) = run(&dir);
    assert_eq!(second_code, 0, "{second}");

    let count = |line: &str| -> String {
        line.split_whitespace()
            .skip_while(|w| !w.contains("assets"))
            .nth(1)
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        count(&first),
        count(&second),
        "the second run must see the same assets as the first:\nfirst:  {first}\nsecond: {second}"
    );
}

#[test]
fn a_budget_config_with_a_typo_does_not_pass_the_build() {
    let dir = temp_dir("budget-typo");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");
    let config = dir.join("budget.json");
    fs::write(&config, br#"{"rules":[{"name":"total","limit":1000}]}"#).expect("write config");

    let out = Command::new(binary())
        .arg(&dir)
        .args(["--budget", config.to_str().expect("path")])
        .arg("--report")
        .arg(dir.join("report.html"))
        .output()
        .expect("the CLI runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code().unwrap_or(-1);
    assert_ne!(code, 0, "a config that disables the gate must not exit 0");
    assert!(
        stderr.contains("unknown field") && stderr.contains("limits"),
        "the error should name the field it wanted: {stderr}"
    );
}

#[test]
fn a_budget_config_with_no_rules_is_an_error() {
    let dir = temp_dir("budget-empty");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");
    let config = dir.join("budget.json");
    fs::write(&config, br#"{"limits":[]}"#).expect("write config");

    let out = Command::new(binary())
        .arg(&dir)
        .args(["--budget", config.to_str().expect("path")])
        .arg("--report")
        .arg(dir.join("report.html"))
        .output()
        .expect("the CLI runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code().unwrap_or(-1), 0, "an empty rule list must not pass");
    assert!(
        stderr.contains("no limits") && stderr.contains("Add at least one"),
        "the error should say what to do about it: {stderr}"
    );
}

#[test]
fn a_map_that_cannot_be_read_is_said_out_loud() {
    let dir = temp_dir("unreadable-map");
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), br#"{"version":3,"sources":["a.ts"],"mapp"#)
        .expect("write truncated map");

    let (stdout, _, code) = run(&dir);
    assert_eq!(code, 0, "one broken map is not a broken build");
    assert!(
        stdout.contains("could not read") && stdout.contains("index.js.map"),
        "an unreadable map must be named: {stdout}"
    );
}

#[test]
fn a_folder_with_no_build_output_at_all_is_an_error() {
    let dir = temp_dir("empty-folder");
    let (_, stderr, code) = run(&dir);
    assert_ne!(code, 0, "an empty folder must not produce a clean report");
    assert!(
        stderr.contains("no build output") || stderr.contains("no stats.json"),
        "the error should say what it looked for: {stderr}"
    );
}

#[test]
fn the_stats_path_still_reports_ghosts_normally() {
    let dir = temp_dir("with-metadata");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");
    fs::write(
        dir.join("stats.json"),
        r#"{"version":"5.90.0",
            "assets":[{"type":"asset","name":"index.js","size":4096,"chunks":[0],"emitted":true}],
            "chunks":[{"id":0,"names":["main"],"files":["index.js"],"size":4096}],
            "modules":[{"id":1,"identifier":"./src/first.js","name":"./src/first.js","size":2048,"chunks":[0],"reasons":[]},
                       {"id":2,"identifier":"./src/ghost.js","name":"./src/ghost.js","size":2048,"chunks":[0],"reasons":[]}]}"#,
    )
    .expect("write stats");

    let (stdout, _, code) = run(&dir);
    assert_eq!(code, 0, "the stats path should still work");
    assert!(
        stdout.contains("ghost"),
        "with a declared graph, ghosts must be reported again: {stdout}"
    );
    assert!(
        !stdout.contains("needs a stats.json"),
        "the stats path has a graph and must not claim it cannot detect ghosts: {stdout}"
    );
}

#[test]
fn a_csv_export_can_be_requested() {
    let dir = temp_dir("csv-export");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");

    let (stdout, stderr, code) = {
        let out = Command::new(binary())
            .arg(&dir)
            .arg("--csv")
            .arg(dir.join("modules.csv"))
            .args(["--mode", "static", "--report"])
            .arg(dir.join("report.html"))
            .output()
            .expect("the CLI runs");
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.code().unwrap_or(-1),
        )
    };
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("csv"), "the summary line names the csv: {stdout}");

    let csv = fs::read_to_string(dir.join("modules.csv")).expect("the csv exists");
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("module_id,name,package,chunks,stat,parsed,gzip,attributed,delta"),
        "the header row comes first"
    );
    assert!(lines.next().is_some(), "one row per module");
}

#[test]
fn server_mode_serves_the_report_and_a_stamp_endpoint() {
    let dir = temp_dir("server-mode");
    let map = two_source_map(2_048);
    fs::write(dir.join("index.js"), vec![b'x'; 4_096]).expect("write bundle");
    fs::write(dir.join("index.js.map"), map.as_bytes()).expect("write map");

    let mut child = Command::new(binary())
        .arg(&dir)
        .args(["--mode", "server", "--port", "0"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("the CLI server starts");

    let stdout = child.stdout.take().expect("stdout");
    let url = {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("the server prints its URL");
        line.split_whitespace()
            .find(|w| w.starts_with("http://"))
            .expect("the URL is announced: {line}")
            .to_string()
    };

    let get = |target: &str| -> (String, String) {
        use std::io::{Read, Write};
        let host = url.trim_start_matches("http://").trim_end_matches('/');
        let mut stream = std::net::TcpStream::connect(host).expect("connect to the server");
        stream
            .write_all(
                format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .expect("write request");
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("read response");
        let status = raw.lines().next().unwrap_or_default().to_string();
        (status, raw)
    };

    let (status, body) = get("/");
    assert!(status.contains("200"), "GET /: {status}");
    assert!(body.contains("<html"), "the report HTML is served");
    assert!(body.contains("/__stamp"), "the page polls the stamp endpoint");

    let (stamp_status, _) = get("/__stamp");
    assert!(stamp_status.contains("200"), "GET /__stamp: {stamp_status}");

    let _ = child.kill();
    let _ = child.wait();
}
