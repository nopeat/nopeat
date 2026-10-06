use std::collections::BTreeMap;

use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};

use super::package_of;
use crate::Result;
use crate::model::{Asset, InputArtifact, PackageRef, SizeDimension, SizeSet, UnifiedBundleGraph};
use crate::stats::recompute_totals;

#[derive(serde::Deserialize, Default)]
struct MetaInput {
    #[serde(default)]
    bytes: u64,
}

#[derive(serde::Deserialize, Default)]
struct MetaOutput {
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    inputs: BTreeMap<String, MetaOutputEntry>,
}

#[derive(serde::Deserialize, Default)]
struct MetaOutputEntry {
    #[serde(rename = "bytesInOutput", default)]
    bytes_in_output: u64,
}

pub(crate) struct MetafileVisitor;

impl<'de> Visitor<'de> for MetafileVisitor {
    type Value = UnifiedBundleGraph;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an esbuild metafile object")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut inputs: BTreeMap<String, MetaInput> = BTreeMap::new();
        let mut outputs: BTreeMap<String, MetaOutput> = BTreeMap::new();

        while let Some(key) = map.next_key::<std::borrow::Cow<str>>()? {
            match key.as_ref() {
                "inputs" => {
                    map.next_value_seed(MapCollector { out: &mut inputs })?;
                }
                "outputs" => {
                    map.next_value_seed(MapCollector { out: &mut outputs })?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        let mut graph = UnifiedBundleGraph::new();
        graph.inputs.push(InputArtifact::EsbuildMetafile);
        graph.totals.size_dimension = SizeDimension::Parsed;

        for (name, input) in &inputs {
            let package = package_of(name);
            let module = crate::model::Module {
                id: name.clone(),
                name: name.clone(),
                issuer: None,
                reasons: Vec::new(),
                package: package.map(|(name, path)| {
                    std::sync::Arc::new(PackageRef { name, version: None, path })
                }),
                chunks: Vec::new(),
                sizes: SizeSet { stat: input.bytes, ..SizeSet::default() },
                attribution_delta: 0,
                sources: Vec::new(),
            };
            graph.modules.insert(name.clone(), module);
        }

        for (name, output) in &outputs {
            graph.assets.push(Asset {
                name: name.clone(),
                size: output.bytes,
                chunks: Vec::new(),
                sizes: SizeSet { stat: output.bytes, ..SizeSet::default() },
            });
            for (input, entry) in &output.inputs {
                if let Some(module) = graph.modules.get_mut(input) {
                    module.sizes.parsed = module.sizes.parsed.max(entry.bytes_in_output);
                }
            }
        }

        recompute_totals(&mut graph);
        Ok(graph)
    }
}

struct MapCollector<'a, K, V> {
    out: &'a mut BTreeMap<K, V>,
}

impl<'de, K, V> DeserializeSeed<'de> for MapCollector<'_, K, V>
where
    K: serde::Deserialize<'de> + Ord,
    V: serde::Deserialize<'de>,
{
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        struct M<'a, K, V> {
            out: &'a mut BTreeMap<K, V>,
        }
        impl<'de, K, V> Visitor<'de> for M<'_, K, V>
        where
            K: serde::Deserialize<'de> + Ord,
            V: serde::Deserialize<'de>,
        {
            type Value = ();
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
                while let Some(key) = map.next_key::<K>()? {
                    let value = map.next_value::<V>()?;
                    self.out.insert(key, value);
                }
                Ok(())
            }
        }
        d.deserialize_map(M { out: self.out })
    }
}

pub fn ingest_metafile(bytes: &[u8]) -> Result<UnifiedBundleGraph> {
    super::ingest_reader(bytes, "esbuild")
}
