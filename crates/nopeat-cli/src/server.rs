use std::process::ExitCode;

use anyhow::{Context, Result};

use nopeat_core::stats;

use crate::cli_args::Cli;
use crate::discover::{discover, input_stamp};
use crate::payload::build_payload;
use crate::pipeline::{apply_excludes, build_graph, dimension_used, dims_selected};
use crate::report;

struct ServerState {
    stamp: u64,
    html: Vec<u8>,
    detail_js: Vec<u8>,
}

pub fn run_server(cli: &Cli) -> Result<ExitCode> {
    let port = cli.port_number()?;
    let listener = std::net::TcpListener::bind(format!("{}:{}", cli.host, port))
        .with_context(|| format!("binding {}:{}", cli.host, port))?;
    let addr = listener.local_addr().with_context(|| "reading the bound address")?;
    let url = format!("http://{addr}/");
    crate::pipeline::emit(
        cli,
        crate::cli_args::Level::Info,
        &format!("nopeat: watching {} — live report at {url} (Ctrl-C to stop)", cli.path.display()),
    );
    let state = std::sync::Arc::new(std::sync::Mutex::new(ServerState {
        stamp: 0,
        html: Vec::new(),
        detail_js: Vec::new(),
    }));
    for stream in listener.incoming() {
        // One failed accept should not end the server, and it is not worth a
        // diagnostic: it is almost always a client that hung up early.
        let Ok(mut stream) = stream else { continue };
        let state = state.clone();
        let cli = cli.clone();
        std::thread::spawn(move || {
            let _ = serve_conn(&mut stream, &state, &cli);
        });
    }
    Ok(ExitCode::SUCCESS)
}

fn serve_conn(
    stream: &mut std::net::TcpStream,
    state: &std::sync::Mutex<ServerState>,
    cli: &Cli,
) -> std::io::Result<()> {
    use std::io::{Read, Write as _};

    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let route =
        req.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();

    let mut respond = |status: &str, ctype: &str, body: Vec<u8>| -> std::io::Result<()> {
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes())?;
        stream.write_all(&body)?;
        stream.flush()
    };

    let mut state = state.lock().unwrap();
    let stamp = input_stamp(&cli.path);
    if state.stamp != stamp {
        match regenerate(cli) {
            Ok((html, detail_js)) => {
                state.html = html.into_bytes();
                state.detail_js = detail_js;
                state.stamp = stamp;
            }
            Err(err) => {
                return respond(
                    "500 Internal Server Error",
                    "text/plain; charset=utf-8",
                    format!("nopeat: {err:#}").into_bytes(),
                );
            }
        }
    }

    if route == "/__stamp" {
        respond("200 OK", "text/plain", state.stamp.to_string().into_bytes())
    } else if route == "/detail.js" {
        if state.detail_js.is_empty() {
            respond("404 Not Found", "text/plain", b"detail is inlined".to_vec())
        } else {
            respond("200 OK", "application/javascript", state.detail_js.clone())
        }
    } else {
        respond("200 OK", "text/html; charset=utf-8", state.html.clone())
    }
}

fn regenerate(cli: &Cli) -> Result<(String, Vec<u8>)> {
    let found = discover(&cli.path)?;
    let mut fusion = None;
    let mut graph = build_graph(cli, &found, &mut fusion)?;
    if !cli.exclude.is_empty() {
        apply_excludes(&mut graph, &cli.exclude)?;
    }
    stats::recompute_totals(&mut graph);

    let dims = dims_selected(cli);
    let mut payload = build_payload(&graph, &found, &dims, cli.default_sizes, false);
    let used = dimension_used(&graph);
    let label = payload.get("target").and_then(|v| v.as_str()).unwrap_or("bundle").to_string();
    let title = cli.title.as_deref().unwrap_or(&label);
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("sizeDimension".into(), used.into());
        obj.insert("compression".into(), cli.compression_algorithm.as_str().into());
    }

    let mut detail_json = Vec::new();
    nopeat_core::report::write_detail(&graph, &mut detail_json)?;
    let mut detail_js = b"window.__OBS_DETAIL__=".to_vec();
    detail_js.extend_from_slice(&detail_json);
    detail_js.extend_from_slice(b";\n");

    let inline = detail_js.len() <= report::INLINE_LIMIT;
    let mut html = report::render(&payload, &label, title, &detail_json, inline)?;
    if !inline {
        html = html.replace(report::DATA_SCRIPT_PLACEHOLDER, "detail.js");
    }

    let stamp = input_stamp(&cli.path);
    let reload = format!(
        "<script>(function p(){{fetch('/__stamp').then(function(r){{return r.text()}}).then(\
         function(t){{if(t.trim()!=='{stamp}'){{location.reload()}}else{{setTimeout(p,1000)}}}})\
         .catch(function(){{setTimeout(p,1000)}})}})()</script>"
    );
    html = html.replace("</body>", &format!("{reload}</body>"));
    Ok((html, if inline { Vec::new() } else { detail_js }))
}
