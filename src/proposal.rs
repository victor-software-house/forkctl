use crate::ledger::patch_kind;
use crate::manifest::Manifest;
use anyhow::{Context, Result};
use askama::Template;

const SHORT_COMMIT: usize = 12;

/// Title and body of the proposal pull request for one candidate.
pub struct ProposalText {
    pub title: String,
    pub body: String,
}

#[derive(Template)]
#[template(path = "proposal-pr.md", escape = "none")]
struct ProposalTemplate {
    branch: String,
    candidate: String,
    downstream_tip: String,
    base_selector: String,
    base_commit: String,
    patches: Vec<PatchRow>,
}

struct PatchRow {
    name: String,
    kind: &'static str,
    purpose: String,
}

pub fn render(manifest: &Manifest, candidate: &str, downstream_tip: &str) -> Result<ProposalText> {
    let branch = &manifest.downstream.branch;
    let short = candidate.get(..SHORT_COMMIT).unwrap_or(candidate);
    let mut body = ProposalTemplate {
        branch: escape_inline(branch),
        candidate: escape_inline(candidate),
        downstream_tip: escape_inline(downstream_tip),
        base_selector: escape_inline(&manifest.base.target.selector),
        base_commit: escape_inline(&manifest.base.target.commit),
        patches: manifest
            .patches
            .iter()
            .map(|patch| PatchRow {
                name: escape_inline(&patch.name),
                kind: patch_kind(patch.kind),
                purpose: patch.purpose.replace('\n', " "),
            })
            .collect(),
    }
    .render()
    .context("render proposal pull request body")?;
    if !body.ends_with('\n') {
        body.push('\n');
    }
    Ok(ProposalText {
        title: format!("forkctl: propose {branch} at {short}"),
        body,
    })
}

/// `HOST/OWNER/REPO` for `gh --repo`, parsed from a remote URL.
///
/// Accepts `https://`, `http://`, and `ssh://` URLs and scp-style `[USER@]HOST:OWNER/REPO`, each
/// with an optional `.git` suffix. Anything else, such as a local path, names no repository.
pub fn github_repo(url: &str) -> Option<String> {
    let (host, path) = if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        if !matches!(scheme, "ssh" | "https" | "http") {
            return None;
        }
        // `gh` knows a host by name, never by port.
        (host.split_once(':').map_or(host, |(host, _)| host), path)
    } else {
        let (authority, path) = url.split_once(':')?;
        if authority.contains('/') {
            return None;
        }
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        (host, path)
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    (!host.is_empty() && !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
        .then(|| format!("{host}/{owner}/{repo}"))
}

/// The URL without the user information of a `scheme://` authority, which can carry a token.
///
/// Everything up to the last `@` is dropped, because an unencoded password may itself contain
/// `/` or `@`. An `@` later in the path over-redacts, which is safe for a message.
pub fn redact_userinfo(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    rest.rsplit_once('@').map_or_else(
        || url.to_string(),
        |(_, after)| format!("{scheme}://{after}"),
    )
}

fn escape_inline(value: &str) -> String {
    value.replace('\\', "\\\\").replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::{github_repo, redact_userinfo};

    #[test]
    fn strips_ports_for_every_scheme() {
        for url in [
            "https://ghe.example.com:8443/example/downstream.git",
            "http://ghe.example.com:8080/example/downstream",
            "ssh://git@ghe.example.com:2222/example/downstream.git",
        ] {
            assert_eq!(
                github_repo(url).as_deref(),
                Some("ghe.example.com/example/downstream"),
                "{url}"
            );
        }
    }

    #[test]
    fn redacts_user_information_only() {
        assert_eq!(
            redact_userinfo("https://user:token@gitlab.example.com/group/sub/repo.git"),
            "https://gitlab.example.com/group/sub/repo.git"
        );
        assert_eq!(
            redact_userinfo("https://token@example.com"),
            "https://example.com"
        );
        assert_eq!(
            redact_userinfo("https://user:ab/cd@host.example.com/group/sub/repo.git"),
            "https://host.example.com/group/sub/repo.git"
        );
        for url in [
            "https://github.com/example/downstream.git",
            "git@github.com:example/downstream.git",
            "/srv/git/fork.git",
        ] {
            assert_eq!(redact_userinfo(url), url);
        }
    }

    #[test]
    fn parses_github_remote_forms() {
        for url in [
            "https://github.com/example/downstream.git",
            "https://token@github.com/example/downstream",
            "ssh://git@github.com/example/downstream.git",
            "ssh://git@github.com:22/example/downstream",
            "git@github.com:example/downstream.git",
            "github.com:example/downstream/",
        ] {
            assert_eq!(
                github_repo(url).as_deref(),
                Some("github.com/example/downstream"),
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_urls_that_name_no_repository() {
        for url in [
            "/srv/git/fork.git",
            "./fork.git",
            "file:///srv/git/fork.git",
            "https://github.com/example",
            "https://github.com/example/downstream/extra",
            "git@github.com:downstream.git",
        ] {
            assert_eq!(github_repo(url), None, "{url}");
        }
    }
}
