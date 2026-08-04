use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn command_and_domain_modules_do_not_render_or_print() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    visit(&root, &mut |path| {
        if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("view.rs" | "help.rs" | "main.rs" | "cli.rs")
        ) {
            return;
        }
        let source = fs::read_to_string(path).unwrap();
        for forbidden in [
            "println!(",
            "eprintln!(",
            "print!(",
            "eprint!(",
            "anstream::",
            "comfy_table::",
            "std::io::stdout",
            "std::io::stderr",
        ] {
            if source.contains(forbidden) {
                violations.push(format!("{} contains {forbidden}", path.display()));
            }
        }
    });
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

fn visit(directory: &Path, inspect: &mut impl FnMut(&Path)) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            visit(&path, inspect);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            inspect(&path);
        }
    }
}
