use crate::error::{AppResult, DomainError};
use crate::manifest::Manifest;
use anyhow::Result;
use ctl_core::input::Input;
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ManifestFormat {
    Yaml,
    Json,
}

impl ManifestFormat {
    pub(crate) fn from_path(path: &Path) -> AppResult<Self> {
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

    pub(crate) fn parse(self, bytes: &[u8], path: &Path) -> AppResult<Manifest> {
        match self {
            Self::Yaml => {
                let text = std::str::from_utf8(bytes).map_err(|error| {
                    DomainError::manifest_invalid(format!(
                        "YAML manifest {} is not UTF-8: {error}",
                        path.display()
                    ))
                })?;
                Input::new(path.display().to_string(), text)
                    .parse()
                    .map_err(|error| DomainError::manifest_invalid(error.to_string()).into())
            }
            Self::Json => serde_json::from_slice(bytes).map_err(|error| {
                DomainError::manifest_invalid(format!(
                    "parse JSON manifest {}: {error}",
                    path.display()
                ))
                .into()
            }),
        }
    }

    pub(crate) fn serialize(self, manifest: &Manifest) -> Result<Vec<u8>> {
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
    fn accepts_anchors_and_aliases() {
        let anchored = EXAMPLE
            .replacen(
                "drop_when: Upstream provides the required behavior.\n",
                "drop_when: &drop Upstream provides the required behavior.\n",
                1,
            )
            .replacen(
                "drop_when: The downstream fork is retired.\n",
                "drop_when: *drop\n",
                1,
            );
        let manifest = ManifestFormat::Yaml
            .parse(anchored.as_bytes(), Path::new("fork.yaml"))
            .unwrap();
        assert_eq!(manifest.patches[1].drop_when, manifest.patches[0].drop_when);
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
