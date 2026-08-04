use crate::error::DomainError;
use crate::manifest::Manifest;
use anyhow::{Context, Result};
use serde_saphyr::RequireIndent;
use serde_saphyr::options::{DuplicateKeyPolicy, MergeKeyPolicy};
use std::path::Path;

const MAX_YAML_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ManifestFormat {
    Yaml,
    Json,
}

impl ManifestFormat {
    pub fn from_path(path: &Path) -> Result<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("yaml" | "yml") => Ok(Self::Yaml),
            Some("json") => Ok(Self::Json),
            extension => Err(DomainError::manifest_invalid(format!(
                "manifest path must end in .yaml, .yml, or .json, found {}",
                extension.unwrap_or("no extension")
            ))
            .into()),
        }
    }

    pub fn parse(self, bytes: &[u8], path: &Path) -> Result<Manifest> {
        match self {
            Self::Yaml => parse_yaml(bytes, path),
            Self::Json => serde_json::from_slice(bytes)
                .with_context(|| format!("parse JSON manifest {}", path.display())),
        }
    }

    pub fn serialize(self, manifest: &Manifest) -> Result<Vec<u8>> {
        let mut bytes = match self {
            Self::Yaml => {
                let options = serde_saphyr::ser_options! {
                    indent_step: 2,
                    compact_list_indent: false,
                    tagged_enums: false,
                    prefer_block_scalars: true,
                    yaml_12: true,
                };
                serde_saphyr::to_string_with_options(manifest, options)?.into_bytes()
            }
            Self::Json => serde_json::to_vec_pretty(manifest)?,
        };
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        Ok(bytes)
    }
}

fn parse_yaml(bytes: &[u8], path: &Path) -> Result<Manifest> {
    if bytes.len() > MAX_YAML_BYTES {
        return Err(DomainError::manifest_invalid(format!(
            "YAML manifest {} exceeds {} bytes",
            path.display(),
            MAX_YAML_BYTES
        ))
        .into());
    }
    let input = std::str::from_utf8(bytes)
        .with_context(|| format!("YAML manifest {} is not UTF-8", path.display()))?;
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_documents: 1,
            max_aliases: 0,
            max_anchors: 0,
            max_depth: 32,
            max_nodes: 100_000,
            max_total_scalar_bytes: MAX_YAML_BYTES,
            max_merge_keys: 0,
            max_inclusion_depth: 0,
        },
        duplicate_keys: DuplicateKeyPolicy::Error,
        merge_keys: MergeKeyPolicy::Error,
        strict_booleans: true,
        require_indent: RequireIndent::Even,
    };
    serde_saphyr::from_str_with_options(input, options)
        .with_context(|| format!("parse YAML manifest {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../examples/fork.yaml");

    #[test]
    fn yaml_and_json_round_trip_the_same_manifest_deterministically() {
        let yaml_path = Path::new("fork.yaml");
        let json_path = Path::new("fork.json");
        let manifest = ManifestFormat::Yaml
            .parse(EXAMPLE.as_bytes(), yaml_path)
            .unwrap();
        let yaml = ManifestFormat::Yaml.serialize(&manifest).unwrap();
        assert_eq!(
            yaml,
            ManifestFormat::Yaml
                .serialize(&ManifestFormat::Yaml.parse(&yaml, yaml_path).unwrap())
                .unwrap()
        );
        let json = ManifestFormat::Json.serialize(&manifest).unwrap();
        let from_json = ManifestFormat::Json.parse(&json, json_path).unwrap();
        assert_eq!(
            serde_json::to_value(&manifest).unwrap(),
            serde_json::to_value(from_json).unwrap()
        );
    }

    #[test]
    fn rejects_ambiguous_yaml_features() {
        let path = Path::new("fork.yaml");
        let duplicate = EXAMPLE.replacen("schema: 1\n", "schema: 1\nschema: 1\n", 1);
        assert!(
            ManifestFormat::Yaml
                .parse(duplicate.as_bytes(), path)
                .is_err()
        );

        let merged = EXAMPLE.replacen(
            "downstream:\n",
            "downstream:\n  <<: { remote: origin }\n",
            1,
        );
        assert!(ManifestFormat::Yaml.parse(merged.as_bytes(), path).is_err());

        let multiple = format!("{EXAMPLE}\n---\nschema: 1\n");
        assert!(
            ManifestFormat::Yaml
                .parse(multiple.as_bytes(), path)
                .is_err()
        );
    }

    #[test]
    fn extension_selects_the_codec_without_sniffing() {
        assert_eq!(
            ManifestFormat::from_path(Path::new("fork.yaml")).unwrap(),
            ManifestFormat::Yaml
        );
        assert_eq!(
            ManifestFormat::from_path(Path::new("fork.yml")).unwrap(),
            ManifestFormat::Yaml
        );
        assert_eq!(
            ManifestFormat::from_path(Path::new("fork.json")).unwrap(),
            ManifestFormat::Json
        );
        assert!(ManifestFormat::from_path(Path::new("fork.toml")).is_err());
        assert!(ManifestFormat::from_path(Path::new("fork")).is_err());
    }
}
