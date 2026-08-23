//! Clap verbs the operator skill never names. Not a list of required sentences.

use crate::cli::Cli;
use clap::CommandFactory;
use std::fs;
use std::path::Path;

const SKILL: &str = "skills/forkctl/SKILL.md";

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn verbs() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .filter(|command| !command.is_hide_set())
        .map(|command| command.get_name().to_string())
        .collect()
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|token| !token.is_empty())
}

fn unnamed<'a>(body: &'a str, verb: &'a str) -> bool {
    !tokens(body).any(|token| token == verb)
}

#[test]
fn skill_names_every_clap_verb() {
    let body = crate_file(SKILL);
    let missing: Vec<String> = verbs()
        .into_iter()
        .filter(|verb| unnamed(&body, verb))
        .collect();
    assert!(
        missing.is_empty(),
        "{SKILL} never names {missing:?} (any mention counts)"
    );
}
