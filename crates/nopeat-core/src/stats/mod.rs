mod ingest;
mod metafile;

pub use ingest::{ingest_bytes, ingest_file, ingest_reader, ingest_stats, recompute_totals};
pub use metafile::ingest_metafile;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawModule {
    pub identifier: Option<String>,

    #[serde(default)]
    pub children: Vec<RawModule>,

    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub chunks: Vec<u32>,
    pub issuer_name: Option<String>,
    #[serde(default)]
    pub reasons: Vec<RawReason>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawReason {
    #[serde(default)]
    pub module_name: Option<String>,
    #[serde(rename = "type", default)]
    pub reason_type: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub chunks: Vec<u32>,
    #[serde(default)]
    pub chunk_names: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawChunk {
    pub id: u32,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub initial: bool,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub size: u64,
}

pub fn join_key(identifier: Option<&str>, name: &str, chunk: Option<u32>) -> String {
    match identifier {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => match chunk {
            Some(c) => format!("{c}:{name}"),
            None => name.to_string(),
        },
    }
}

pub fn package_of(name: &str) -> Option<(String, String)> {
    let normalized = name.replace('\\', "/");
    let idx = normalized.rfind("node_modules/")?;
    let rest = &normalized[idx + "node_modules/".len()..];
    let mut segments = rest.split('/');
    let first = segments.next()?;
    if first.is_empty() {
        return None;
    }
    let (pkg, path) = if first.starts_with('@') {
        match segments.next() {
            Some(second) if !second.is_empty() => {
                (format!("{first}/{second}"), format!("node_modules/{first}/{second}"))
            }
            _ => return None,
        }
    } else {
        (first.to_string(), format!("node_modules/{first}"))
    };
    Some((pkg, path))
}

#[cfg(test)]
mod tests {
    use super::{join_key, package_of};

    #[test]
    fn identifier_wins() {
        assert_eq!(join_key(Some("/repo/a.js"), "a.js", Some(3)), "/repo/a.js");
    }

    #[test]
    fn name_is_chunk_scoped() {
        assert_eq!(join_key(None, "./src/a.js", Some(3)), "3:./src/a.js");
        assert_eq!(join_key(None, "./src/a.js", Some(9)), "9:./src/a.js");
    }

    #[test]
    fn packages_are_found_behind_node_modules() {
        assert_eq!(
            package_of("./node_modules/preact/compat/src/index.js"),
            Some(("preact".into(), "node_modules/preact".into()))
        );
        assert_eq!(
            package_of("./node_modules/@scope/pkg/dist/i.js"),
            Some(("@scope/pkg".into(), "node_modules/@scope/pkg".into()))
        );
        assert_eq!(package_of("./src/app.js"), None);
    }

    #[test]
    fn windows_separators_are_handled() {
        assert_eq!(
            package_of(".\\node_modules\\marked\\lib\\marked.js"),
            Some(("marked".into(), "node_modules/marked".into()))
        );
    }

    #[test]
    fn nested_modules_under_a_nameless_container_are_ingested() {
        let stats = br#"{
            "modules": [
                {
                    "type": "modules",
                    "size": 300,
                    "children": [
                        {"name": "./src/a.ts", "size": 100, "chunks": [1]},
                        {"name": "./src/b.ts", "size": 200, "chunks": [1]}
                    ]
                },
                {
                    "type": "runtime modules",
                    "size": 20,
                    "children": [{"name": "webpack/runtime/define property getters", "size": 20}]
                },
                {"moduleType": "runtime", "size": 7}
            ]
        }"#;

        let graph = crate::stats::ingest::ingest_stats(stats, "webpack").expect("valid stats");

        assert_eq!(graph.modules.len(), 3, "two app modules and one runtime module");
        assert!(graph.modules.values().any(|m| m.name == "./src/a.ts"));
        assert!(graph.modules.values().any(|m| m.name == "./src/b.ts"));

        let total: u64 = graph.modules.values().map(|m| m.sizes.stat).sum();
        assert_eq!(total, 320, "container sizes must not be counted again");

        assert!(
            graph.diagnostics.iter().any(|d| d.code == "NPT0052"),
            "expected NPT0052 for the nameless runtime module, got {:?}",
            graph.diagnostics.iter().map(|d| d.code.as_str()).collect::<Vec<_>>()
        );
    }
}
