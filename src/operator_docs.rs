//! Committed operator documents rendered from Clap and consumer-owned prose.

use crate::cli::Cli;
use std::fs;
use std::path::Path;

const SKILL_TEMPLATE: &str = ".ctl/operator/SKILL.md.jinja";
const INSTRUCTIONS_TEMPLATE: &str = ".ctl/operator/instructions.md.jinja";
const SKILL: &str = "skills/forkctl/SKILL.md";
const INSTRUCTIONS: &str = "src/instructions.md";

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn render_template(relative: &str) -> String {
    let source = crate_file(relative);
    let environment = ctl_core::surface::environment()
        .unwrap_or_else(|error| panic!("operator environment: {error}"));
    environment
        .template_from_named_str(relative, &source)
        .unwrap_or_else(|error| panic!("parse {relative}: {error}"))
        .render(minijinja::context! {
            surface => ctl_core::Surface::new::<Cli>("fork"),
        })
        .unwrap_or_else(|error| panic!("render {relative}: {error}"))
}

fn assert_committed(relative: &str, rendered: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if std::env::var_os("UPDATE_OPERATOR_DOCS").is_some() {
        fs::write(&path, rendered)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    }
    assert_eq!(rendered, crate_file(relative));
}

#[test]
fn skill_is_the_committed_surface_render() {
    assert_committed(SKILL, &render_template(SKILL_TEMPLATE));
}

#[test]
fn instructions_are_the_committed_surface_render() {
    assert_committed(INSTRUCTIONS, &render_template(INSTRUCTIONS_TEMPLATE));
}
