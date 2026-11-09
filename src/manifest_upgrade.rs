//! The one upgrade from manifest schema 1, which named three fields `kind`:
//! a patch's layer, a base target's type, and a history entry's event.
//! Schema 2 names them `layer`, `type`, and `event`.
//!
//! `forkctl upgrade` and the hint in `App::discover` are the only readers of
//! schema 1. Both go, with this module, in the change that ends support for
//! upgrading from forkctl 0.0.48.

use std::path::Path;

use ctl_core::input::Input;
use serde_json::Value;
use yamled::{Document, Path as YamlPath};

use crate::error::{AppError, AppResult as Result, DomainError};
use crate::manifest::{Manifest, SCHEMA};
use crate::manifest_codec::ManifestFormat;

/// The schema this module upgrades from.
pub(crate) const EARLIER_SCHEMA: u32 = 1;

/// One step from the manifest root to a mapping.
#[derive(Debug, Clone, Copy)]
enum Step {
    Key(&'static str),
    Index(usize),
}

/// A mapping whose `kind` key becomes `to`.
#[derive(Debug)]
struct Rename {
    at: Vec<Step>,
    to: &'static str,
}

/// The schema a manifest's text declares, read without its typed form.
pub(crate) fn declared_schema(format: ManifestFormat, text: &str, path: &Path) -> Result<u64> {
    generic(format, text, path)?
        .get("schema")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            DomainError::manifest_invalid(format!("{} declares no schema", path.display())).into()
        })
}

/// Rewrite a schema 1 manifest as schema 2, in the same format. A YAML
/// manifest keeps every byte other than the renamed keys and the schema
/// number; a JSON manifest is written in full, as every JSON write is.
pub(crate) fn upgrade(format: ManifestFormat, text: &str, path: &Path) -> Result<String> {
    let value = generic(format, text, path)?;
    let schema = value.get("schema").and_then(Value::as_u64);
    if schema != Some(u64::from(EARLIER_SCHEMA)) {
        return Err(DomainError::manifest_invalid(format!(
            "{} is not manifest schema {EARLIER_SCHEMA}; forkctl upgrade reads only that schema",
            path.display()
        ))
        .into());
    }
    let renames = renames(&value);
    match format {
        ManifestFormat::Yaml => upgrade_yaml(text, path, &renames),
        ManifestFormat::Json => upgrade_json(value, path, &renames),
    }
}

fn generic(format: ManifestFormat, text: &str, path: &Path) -> Result<Value> {
    match format {
        ManifestFormat::Yaml => Input::new(path.display().to_string(), text)
            .parse()
            .map_err(|error| DomainError::manifest_invalid(error.to_string()).into()),
        ManifestFormat::Json => serde_json::from_str(text).map_err(|error| {
            DomainError::manifest_invalid(format!(
                "parse JSON manifest {}: {error}",
                path.display()
            ))
            .into()
        }),
    }
}

/// Every mapping in a schema 1 manifest that holds a `kind` key, by record.
fn renames(manifest: &Value) -> Vec<Rename> {
    let mut renames = vec![Rename {
        at: vec![Step::Key("base"), Step::Key("target")],
        to: "type",
    }];
    for index in 0..len(manifest, "patches") {
        renames.push(Rename {
            at: vec![Step::Key("patches"), Step::Index(index)],
            to: "layer",
        });
    }
    for index in 0..len(manifest, "disabled_patches") {
        renames.push(Rename {
            at: vec![
                Step::Key("disabled_patches"),
                Step::Index(index),
                Step::Key("patch"),
            ],
            to: "layer",
        });
    }
    let history = manifest.get("history").and_then(Value::as_array);
    for (index, entry) in history.into_iter().flatten().enumerate() {
        let at = |steps: &[Step]| {
            [Step::Key("history"), Step::Index(index)]
                .into_iter()
                .chain(steps.iter().copied())
                .collect::<Vec<_>>()
        };
        renames.push(Rename {
            at: at(&[]),
            to: "event",
        });
        match entry.get("kind").and_then(Value::as_str) {
            Some("rebase") => {
                renames.push(Rename {
                    at: at(&[Step::Key("target")]),
                    to: "type",
                });
                for dropped in 0..len(entry, "dropped") {
                    renames.push(Rename {
                        at: at(&[
                            Step::Key("dropped"),
                            Step::Index(dropped),
                            Step::Key("patch"),
                        ]),
                        to: "layer",
                    });
                }
            }
            Some("patch_removed" | "patch_enabled") => renames.push(Rename {
                at: at(&[Step::Key("record"), Step::Key("patch")]),
                to: "layer",
            }),
            // The typed parse of the result names an unknown event.
            Some(_) | None => {}
        }
    }
    renames
}

fn len(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

/// Splice each renamed key and the schema number into the source text.
fn upgrade_yaml(text: &str, path: &Path, renames: &[Rename]) -> Result<String> {
    let document = Document::parse(text).map_err(|error| refused(path, &error))?;
    let schema = document
        .node(&YamlPath::root().key("schema"))
        .map(|node| node.value())
        .ok_or_else(|| {
            DomainError::manifest_invalid(format!("{} declares no schema", path.display()))
        })?;
    let mut splices = vec![(schema.start, schema.end, SCHEMA.to_string())];
    for rename in renames {
        let key = rename
            .at
            .iter()
            .fold(YamlPath::root(), |path, step| match step {
                Step::Key(key) => path.key(*key),
                Step::Index(index) => path.index(*index),
            })
            .key("kind");
        // An alias shares its anchor's key, which another rename reaches.
        if let Some(location) = document.node(&key).and_then(|node| node.key()) {
            splices.push((location.start, location.end, rename.to.to_string()));
        }
    }
    splices.sort_by_key(|splice| std::cmp::Reverse(splice.0));
    splices.dedup_by_key(|splice| splice.0);
    let mut upgraded = text.to_string();
    for (start, end, replacement) in splices {
        upgraded.replace_range(start..end, &replacement);
    }
    Ok(upgraded)
}

fn upgrade_json(mut value: Value, path: &Path, renames: &[Rename]) -> Result<String> {
    if let Some(schema) = value.get_mut("schema") {
        *schema = Value::from(SCHEMA);
    }
    for rename in renames {
        let mapping = rename
            .at
            .iter()
            .try_fold(&mut value, |value, step| match step {
                Step::Key(key) => value.get_mut(*key),
                Step::Index(index) => value.get_mut(*index),
            })
            .and_then(Value::as_object_mut);
        if let Some(mapping) = mapping
            && let Some(renamed) = mapping.remove("kind")
        {
            mapping.insert(rename.to.to_string(), renamed);
        }
    }
    let manifest: Manifest = serde_json::from_value(value).map_err(|error| {
        DomainError::manifest_invalid(format!(
            "{} does not read as manifest schema {SCHEMA} after the upgrade: {error}",
            path.display()
        ))
    })?;
    let bytes = ManifestFormat::Json
        .serialize(&manifest)
        .map_err(|error| AppError::internal(error, format!("serialize {}", path.display())))?;
    String::from_utf8(bytes).map_err(|error| AppError::internal(error, "JSON manifest is UTF-8"))
}

fn refused(path: &Path, error: &yamled::Error) -> AppError {
    DomainError::manifest_invalid(format!(
        "cannot upgrade {} in place: {error}",
        path.display()
    ))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA_1: &str = include_str!("../tests/testdata/upgrade/schema-1.yaml");
    const SCHEMA_2: &str = include_str!("../tests/testdata/upgrade/schema-2.yaml");

    #[test]
    fn yaml_upgrade_renames_only_the_kind_keys_and_the_schema() {
        let path = Path::new("fork.yaml");
        let upgraded = upgrade(ManifestFormat::Yaml, SCHEMA_1, path).unwrap();
        assert_eq!(upgraded, SCHEMA_2);
        ManifestFormat::Yaml
            .parse(upgraded.as_bytes(), path)
            .unwrap();
    }

    #[test]
    fn json_upgrade_reads_as_the_yaml_upgrade() {
        let yaml_path = Path::new("fork.yaml");
        let json_path = Path::new("fork.json");
        let schema_1 = serde_json::to_string_pretty(
            &generic(ManifestFormat::Yaml, SCHEMA_1, yaml_path).unwrap(),
        )
        .unwrap();
        let upgraded = upgrade(ManifestFormat::Json, &schema_1, json_path).unwrap();
        let from_json = ManifestFormat::Json
            .parse(upgraded.as_bytes(), json_path)
            .unwrap();
        let from_yaml = ManifestFormat::Yaml
            .parse(SCHEMA_2.as_bytes(), yaml_path)
            .unwrap();
        assert_eq!(
            serde_json::to_value(from_json).unwrap(),
            serde_json::to_value(from_yaml).unwrap()
        );
    }

    #[test]
    fn upgrade_reads_only_schema_1() {
        let path = Path::new("fork.yaml");
        assert_eq!(
            declared_schema(ManifestFormat::Yaml, SCHEMA_2, path).unwrap(),
            u64::from(SCHEMA)
        );
        assert!(upgrade(ManifestFormat::Yaml, SCHEMA_2, path).is_err());
    }
}
