use anyhow::{Result, ensure};
use clap::ValueEnum;
use globset::GlobBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub downstream: Downstream,
    pub upstream: Upstream,
    pub base: Base,
    pub documents: Documents,
    pub bookkeeping_patch: String,
    #[serde(default = "CommitMessagePolicy::legacy_default")]
    #[schemars(required)]
    pub commit_messages: CommitMessagePolicy,
    pub patches: Vec<Patch>,
    #[serde(default)]
    pub disabled_patches: Vec<DisabledPatch>,
    #[serde(default)]
    pub history: Vec<HistoryEvent>,
    #[serde(default)]
    pub contracts: Contracts,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Downstream {
    pub remote: String,
    pub branch: String,
    pub recovery_tag_prefix: String,
    /// How `publish` moves this branch when no CLI flag is given.
    #[serde(default, skip_serializing_if = "skip_rewrite_mode")]
    pub publish: PublishMode,
}

/// How a repository publishes the managed downstream branch.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    Deserialize,
    Eq,
    PartialEq,
    schemars::JsonSchema,
    Serialize,
    ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum PublishMode {
    /// Exact-lease rewrite of the downstream ref. Recovery tags keep the overwritten tip.
    #[default]
    Rewrite,
    /// Keep the previous tip as an ancestor and fast-forward. `git log` stays a line.
    Append,
    /// Open a proposal of the net tree. `publish --promote` later moves the lease.
    Propose,
}

impl PublishMode {
    pub fn is_rewrite(self) -> bool {
        matches!(self, Self::Rewrite)
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn skip_rewrite_mode(mode: &PublishMode) -> bool {
    mode.is_rewrite()
}

impl std::fmt::Display for PublishMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Rewrite => "rewrite",
            Self::Append => "append",
            Self::Propose => "propose",
        })
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub remote: String,
    pub url: String,
    pub fetch_ref: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Base {
    pub target: BaseTarget,
    pub canonical: String,
    pub stack: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Documents {
    pub ledger: String,
    pub exports: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaseTarget {
    pub kind: TargetKind,
    pub selector: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_object: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Commit,
    Branch,
    Tag,
}

#[derive(
    Debug, Clone, Copy, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize, ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum PatchKind {
    Source,
    Tooling,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommitConvention {
    #[serde(rename = "type")]
    pub commit_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommitMessagePolicy {
    pub source: CommitConvention,
    pub tooling: CommitConvention,
    #[serde(skip)]
    #[schemars(skip)]
    declared: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitMessagePolicyWire {
    source: CommitConvention,
    tooling: CommitConvention,
}

impl<'de> Deserialize<'de> for CommitMessagePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let policy = CommitMessagePolicyWire::deserialize(deserializer)?;
        Ok(Self {
            source: policy.source,
            tooling: policy.tooling,
            declared: true,
        })
    }
}

impl Default for CommitMessagePolicy {
    fn default() -> Self {
        Self::with_declared(true)
    }
}

impl CommitMessagePolicy {
    fn with_declared(declared: bool) -> Self {
        Self {
            source: CommitConvention {
                commit_type: "feat".into(),
                scope: None,
            },
            tooling: CommitConvention {
                commit_type: "chore".into(),
                scope: Some("tooling".into()),
            },
            declared,
        }
    }

    fn legacy_default() -> Self {
        Self::with_declared(false)
    }

    pub fn is_declared(&self) -> bool {
        self.declared
    }

    pub fn mark_declared(&mut self) {
        self.declared = true;
    }

    pub fn validate(&self) -> Result<()> {
        self.source.validate("source commit convention")?;
        self.tooling.validate("tooling commit convention")
    }

    fn convention(&self, kind: PatchKind) -> &CommitConvention {
        match kind {
            PatchKind::Source => &self.source,
            PatchKind::Tooling => &self.tooling,
        }
    }
}

impl CommitConvention {
    fn validate(&self, label: &str) -> Result<()> {
        ensure!(
            valid_commit_token(&self.commit_type),
            "invalid {label} type: {:?}",
            self.commit_type
        );
        if let Some(scope) = &self.scope {
            ensure!(
                valid_commit_token(scope),
                "invalid {label} scope: {scope:?}"
            );
        }
        Ok(())
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let (commit_type, scope) = parse_conventional_prefix(value)?;
        let convention = Self {
            commit_type: commit_type.to_string(),
            scope: scope.map(str::to_string),
        };
        convention
            .validate("commit convention")
            .map_err(|error| error.to_string())?;
        Ok(convention)
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchCommitMessage {
    #[serde(rename = "type")]
    pub commit_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub description: String,
}

impl PatchCommitMessage {
    fn validate(&self, patch: &str) -> Result<()> {
        CommitConvention {
            commit_type: self.commit_type.clone(),
            scope: self.scope.clone(),
        }
        .validate(&format!("commit override for patch {patch}"))?;
        ensure!(
            valid_commit_description(&self.description),
            "invalid commit description for patch {patch}: {:?}",
            self.description
        );
        Ok(())
    }

    fn subject(&self) -> String {
        conventional_subject(&self.commit_type, self.scope.as_deref(), &self.description)
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let (prefix, description) = value
            .split_once(": ")
            .ok_or_else(|| "commit subject must use `type(scope): description`".to_string())?;
        let (commit_type, scope) = parse_conventional_prefix(prefix)?;
        let message = Self {
            commit_type: commit_type.to_string(),
            scope: scope.map(str::to_string),
            description: description.to_string(),
        };
        message
            .validate("CLI override")
            .map_err(|error| error.to_string())?;
        Ok(message)
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub name: String,
    pub kind: PatchKind,
    pub purpose: String,
    pub upstream_status: String,
    pub drop_when: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<PatchCommitMessage>,
    pub scope: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
}

/// One command that must succeed for the declaring patch to still hold.
///
/// `glob` defaults to the declaring patch's scope and may reach anywhere in the repository: the
/// drift that breaks a long-lived fork is upstream introducing cases the patch never covered, in
/// files the patch does not own. Scope governs what a patch may modify, never what it may check.
#[derive(Debug, Clone, Deserialize, Eq, Hash, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub name: String,
    pub run: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glob: Vec<String>,
    #[serde(default)]
    pub at: CheckStage,
}

/// Which applied tree a check observes.
#[derive(
    Debug, Clone, Copy, Default, Deserialize, Eq, Hash, PartialEq, schemars::JsonSchema, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckStage {
    /// The complete applied stack, in a disposable clone with no origin remote.
    #[default]
    Stack,
    /// The declaring patch's own commit, in a disposable clone with no origin remote.
    Patch,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistoryEvent {
    Rebase {
        target: BaseTarget,
        recovery: RecoveryEvidence,
        dropped: Vec<DroppedPatch>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path_changes: Vec<ReplayPathChange>,
    },
    PatchRemoved {
        record: DisabledPatch,
    },
    PatchEnabled {
        record: DisabledPatch,
        recovery: RecoveryEvidence,
    },
}

/// A surviving patch whose touched paths changed across replay.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayPathChange {
    pub patch: String,
    pub commit: String,
    pub lost_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryEvidence {
    pub tag: String,
    pub tag_object: String,
    pub old_base: String,
    pub old_tip: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DroppedPatch {
    pub patch: Patch,
    pub commit: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisabledPatch {
    pub patch: Patch,
    pub commit: String,
    pub position: usize,
    pub reason: String,
    pub recovery: RecoveryEvidence,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Contracts {
    pub allow_base: Vec<String>,
    pub required_text: Vec<RequiredText>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredText {
    pub path: String,
    pub contains: String,
}

impl Manifest {
    pub fn recovery_evidence(&self) -> Vec<&RecoveryEvidence> {
        let mut recoveries = self
            .disabled_patches
            .iter()
            .map(|record| &record.recovery)
            .collect::<Vec<_>>();
        for event in &self.history {
            match event {
                HistoryEvent::Rebase { recovery, .. } => recoveries.push(recovery),
                HistoryEvent::PatchRemoved { record } => recoveries.push(&record.recovery),
                HistoryEvent::PatchEnabled { record, recovery } => {
                    recoveries.push(&record.recovery);
                    recoveries.push(recovery);
                }
            }
        }
        recoveries
    }

    pub fn validate(&self, repo: &Path, manifest_path: &Path) -> Result<()> {
        self.validate_identity()?;
        self.commit_messages.validate()?;
        self.validate_patches()?;
        self.validate_disabled_patches()?;
        self.validate_bookkeeping(repo, manifest_path)?;
        self.validate_history()?;
        self.validate_contracts(repo)?;
        Ok(())
    }

    fn validate_identity(&self) -> Result<()> {
        ensure!(
            self.schema == 1,
            "unsupported manifest schema: {}",
            self.schema
        );
        for (label, value) in [
            ("downstream remote", self.downstream.remote.as_str()),
            ("downstream branch", self.downstream.branch.as_str()),
            (
                "recovery tag prefix",
                self.downstream.recovery_tag_prefix.as_str(),
            ),
            ("upstream remote", self.upstream.remote.as_str()),
            ("upstream URL", self.upstream.url.as_str()),
            ("upstream fetch ref", self.upstream.fetch_ref.as_str()),
            ("ledger", self.documents.ledger.as_str()),
            ("exports directory", self.documents.exports.as_str()),
            ("bookkeeping patch", self.bookkeeping_patch.as_str()),
        ] {
            ensure!(!value.trim().is_empty(), "{label} is required");
        }
        ensure!(
            self.upstream.fetch_ref.starts_with("refs/heads/"),
            "upstream fetch_ref must be a full branch ref"
        );
        ensure!(
            !self.downstream.recovery_tag_prefix.starts_with("refs/")
                && !self.downstream.recovery_tag_prefix.ends_with('/'),
            "recovery_tag_prefix must be a tag-name prefix without refs/tags"
        );
        for (label, value) in [
            ("canonical base", self.base.canonical.as_str()),
            ("stack base", self.base.stack.as_str()),
        ] {
            ensure!(is_full_sha(value), "{label} must be a full commit SHA");
        }
        self.base.target.validate()?;
        ensure!(
            self.base.target.commit == self.base.stack,
            "base target commit must equal stack base"
        );
        validate_repo_path_text(&self.documents.ledger)?;
        validate_repo_path_text(&self.documents.exports)?;
        Ok(())
    }

    fn validate_patches(&self) -> Result<()> {
        ensure!(!self.patches.is_empty(), "at least one patch is required");
        let mut names = HashSet::new();
        let mut seen_tooling = false;
        for patch in &self.patches {
            patch.validate()?;
            ensure!(
                names.insert(&patch.name),
                "duplicate patch name: {}",
                patch.name
            );
            match patch.kind {
                PatchKind::Source => ensure!(
                    !seen_tooling,
                    "source patch {} appears after tooling",
                    patch.name
                ),
                PatchKind::Tooling => seen_tooling = true,
            }
        }
        Ok(())
    }

    fn validate_disabled_patches(&self) -> Result<()> {
        let active = self
            .patches
            .iter()
            .map(|patch| patch.name.as_str())
            .collect::<HashSet<_>>();
        let mut disabled = HashSet::new();
        for record in &self.disabled_patches {
            record.patch.validate()?;
            ensure!(
                disabled.insert(record.patch.name.as_str()),
                "duplicate disabled patch: {}",
                record.patch.name
            );
            ensure!(
                !active.contains(record.patch.name.as_str()),
                "patch cannot be active and disabled: {}",
                record.patch.name
            );
            ensure!(
                is_full_sha(&record.commit),
                "disabled patch commit must be a full SHA"
            );
            ensure!(
                !record.reason.trim().is_empty(),
                "disabled patch reason is required"
            );
            record
                .recovery
                .validate(&self.downstream.recovery_tag_prefix)?;
        }
        Ok(())
    }

    fn validate_bookkeeping(&self, repo: &Path, manifest_path: &Path) -> Result<()> {
        let manifest_relative = manifest_path.strip_prefix(repo)?.to_string_lossy();
        validate_repo_path(repo, &manifest_relative)?;
        validate_repo_path(repo, &self.documents.ledger)?;
        validate_repo_path(repo, &self.documents.exports)?;
        let bookkeeping = self.patches.last().expect("patches validated as non-empty");
        ensure!(
            bookkeeping.name == self.bookkeeping_patch,
            "bookkeeping patch {} must be final",
            self.bookkeeping_patch
        );
        ensure!(
            bookkeeping.kind == PatchKind::Tooling,
            "bookkeeping patch must be tooling"
        );
        for (label, path) in [
            ("ledger", self.documents.ledger.as_str()),
            ("manifest", manifest_relative.as_ref()),
        ] {
            ensure!(
                bookkeeping.owns(path),
                "bookkeeping patch must own {label} {path}"
            );
        }
        for export in self.source_exports() {
            ensure!(
                bookkeeping.owns(&export.path),
                "bookkeeping patch must own export {}",
                export.path
            );
            ensure!(
                export.path != manifest_relative && export.path != self.documents.ledger,
                "export collides with manifest or ledger: {}",
                export.path
            );
            ensure!(
                !self
                    .contracts
                    .required_text
                    .iter()
                    .any(|required| required.path == export.path),
                "export collides with required contract: {}",
                export.path
            );
            for owner in self
                .patches
                .iter()
                .filter(|candidate| candidate.name != self.bookkeeping_patch)
            {
                ensure!(
                    !owner.owns(&export.path),
                    "export {} overlaps patch {}",
                    export.path,
                    owner.name
                );
            }
        }
        Ok(())
    }

    fn validate_history(&self) -> Result<()> {
        let mut identities = HashSet::new();
        for event in &self.history {
            match event {
                HistoryEvent::Rebase {
                    target,
                    recovery,
                    dropped,
                    path_changes,
                } => {
                    target.validate()?;
                    recovery.validate(&self.downstream.recovery_tag_prefix)?;
                    ensure!(
                        !dropped.is_empty() || !path_changes.is_empty(),
                        "rebase history must contain dropped patches or replay path changes"
                    );
                    for item in dropped {
                        item.patch.validate()?;
                        ensure!(
                            is_full_sha(&item.commit),
                            "history commit must be a full SHA"
                        );
                        ensure!(
                            identities.insert((item.patch.name.as_str(), item.commit.as_str())),
                            "duplicate history event for {} at {}",
                            item.patch.name,
                            item.commit
                        );
                    }
                    for item in path_changes {
                        ensure!(
                            valid_patch_name(&item.patch),
                            "invalid replayed patch name: {:?}",
                            item.patch
                        );
                        ensure!(
                            is_full_sha(&item.commit),
                            "replayed patch history commit must be a full SHA"
                        );
                        ensure!(
                            !item.lost_paths.is_empty(),
                            "replayed patch {} must record lost paths",
                            item.patch
                        );
                        for path in &item.lost_paths {
                            validate_repo_path_text(path)?;
                        }
                        ensure!(
                            identities.insert((item.patch.as_str(), item.commit.as_str())),
                            "duplicate history event for {} at {}",
                            item.patch,
                            item.commit
                        );
                    }
                }
                HistoryEvent::PatchRemoved { record } => {
                    record.patch.validate()?;
                    record
                        .recovery
                        .validate(&self.downstream.recovery_tag_prefix)?;
                    ensure!(
                        is_full_sha(&record.commit),
                        "history commit must be a full SHA"
                    );
                    ensure!(
                        !record.reason.trim().is_empty(),
                        "history reason is required"
                    );
                    ensure!(
                        identities.insert((record.patch.name.as_str(), record.commit.as_str())),
                        "duplicate history event for {} at {}",
                        record.patch.name,
                        record.commit
                    );
                }
                HistoryEvent::PatchEnabled { record, recovery } => {
                    record.patch.validate()?;
                    record
                        .recovery
                        .validate(&self.downstream.recovery_tag_prefix)?;
                    recovery.validate(&self.downstream.recovery_tag_prefix)?;
                    ensure!(
                        is_full_sha(&record.commit),
                        "history commit must be a full SHA"
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_contracts(&self, repo: &Path) -> Result<()> {
        for pattern in &self.contracts.allow_base {
            validate_scope(pattern)?;
        }
        for required in &self.contracts.required_text {
            validate_repo_path(repo, &required.path)?;
            ensure!(
                valid_metadata(&required.contains),
                "invalid required text for {}",
                required.path
            );
        }
        Ok(())
    }

    pub fn patch_names(&self) -> Vec<String> {
        self.patches
            .iter()
            .map(|patch| patch.name.clone())
            .collect()
    }

    pub fn patch(&self, name: &str) -> Option<&Patch> {
        self.patches.iter().find(|patch| patch.name == name)
    }

    pub fn check_count(&self) -> usize {
        self.patches.iter().map(|patch| patch.checks.len()).sum()
    }

    pub fn insertion_index(&self, patch: &Patch) -> usize {
        if patch.name == self.bookkeeping_patch {
            return self.patches.len();
        }
        match patch.kind {
            PatchKind::Source => self
                .patches
                .iter()
                .position(|patch| patch.kind == PatchKind::Tooling)
                .unwrap_or(self.patches.len()),
            PatchKind::Tooling => self.patches.len() - 1,
        }
    }

    pub fn export_path(&self, index: usize, patch: &Patch) -> Option<String> {
        (patch.kind == PatchKind::Source).then(|| {
            format!(
                "{}/{:04}-{}.patch",
                self.documents.exports.trim_end_matches('/'),
                index + 1,
                patch.name
            )
        })
    }

    pub fn source_exports(&self) -> Vec<SourceExport<'_>> {
        self.patches
            .iter()
            .enumerate()
            .filter_map(|(index, patch)| {
                self.export_path(index, patch)
                    .map(|path| SourceExport { patch, path })
            })
            .collect()
    }
}

pub struct SourceExport<'a> {
    pub patch: &'a Patch,
    pub path: String,
}

impl BaseTarget {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            is_full_sha(&self.commit),
            "target commit must be a full SHA"
        );
        match self.kind {
            TargetKind::Commit => {
                ensure!(
                    self.selector == self.commit,
                    "commit selector must equal target commit"
                );
                ensure!(
                    self.tag_object.is_none(),
                    "commit target cannot have tag_object"
                );
            }
            TargetKind::Branch => {
                ensure!(
                    self.selector.starts_with("refs/heads/"),
                    "branch selector must be a full refs/heads ref"
                );
                ensure!(
                    self.tag_object.is_none(),
                    "branch target cannot have tag_object"
                );
            }
            TargetKind::Tag => {
                ensure!(
                    self.selector.starts_with("refs/tags/"),
                    "tag selector must be a full refs/tags ref"
                );
                if let Some(object) = &self.tag_object {
                    ensure!(is_full_sha(object), "tag_object must be a full object ID");
                }
            }
        }
        Ok(())
    }
}

impl Patch {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            valid_patch_name(&self.name),
            "invalid patch name: {:?}",
            self.name
        );
        for (label, value) in [
            ("purpose", self.purpose.as_str()),
            ("upstream_status", self.upstream_status.as_str()),
            ("drop_when", self.drop_when.as_str()),
        ] {
            ensure!(
                valid_metadata(value),
                "invalid {label} for patch {}",
                self.name
            );
        }
        ensure!(
            !self.scope.is_empty(),
            "patch {} must declare scope",
            self.name
        );
        let mut scope = HashSet::new();
        for pattern in &self.scope {
            validate_scope(pattern)?;
            ensure!(
                scope.insert(pattern),
                "duplicate scope {pattern} in patch {}",
                self.name
            );
        }
        if let Some(commit) = &self.commit {
            commit.validate(&self.name)?;
        } else {
            let description = normalized_patch_description(&self.name);
            ensure!(
                valid_commit_description(&description),
                "patch {} cannot produce a valid default commit description",
                self.name
            );
        }
        self.validate_checks()?;
        Ok(())
    }

    fn validate_checks(&self) -> Result<()> {
        let mut names = HashSet::new();
        for check in &self.checks {
            ensure!(
                valid_patch_name(&check.name),
                "invalid check name in patch {}: {:?}",
                self.name,
                check.name
            );
            ensure!(
                names.insert(&check.name),
                "duplicate check {} in patch {}",
                check.name,
                self.name
            );
            ensure!(
                valid_metadata(&check.run),
                "check {} in patch {} must declare one command line",
                check.name,
                self.name
            );
            let mut globs = HashSet::new();
            for pattern in &check.glob {
                validate_scope(pattern)?;
                ensure!(
                    globs.insert(pattern),
                    "duplicate glob {pattern} in check {} of patch {}",
                    check.name,
                    self.name
                );
            }
        }
        Ok(())
    }

    /// Files a check observes: its own globs, or the declaring patch's scope by default.
    pub fn check_globs<'a>(&'a self, check: &'a Check) -> &'a [String] {
        if check.glob.is_empty() {
            &self.scope
        } else {
            &check.glob
        }
    }

    pub fn owns(&self, path: &str) -> bool {
        self.scope
            .iter()
            .any(|pattern| scope_matches(pattern, path))
    }

    pub fn message(&self, policy: &CommitMessagePolicy) -> String {
        format!(
            "{}\n\nDownstream-Reason: {}\nUpstream-Status: {}\nDrop-When: {}",
            self.subject(policy),
            self.purpose,
            self.upstream_status,
            self.drop_when
        )
    }

    pub fn subject(&self, policy: &CommitMessagePolicy) -> String {
        if let Some(commit) = &self.commit {
            return commit.subject();
        }
        let convention = policy.convention(self.kind);
        conventional_subject(
            &convention.commit_type,
            convention.scope.as_deref(),
            &normalized_patch_description(&self.name),
        )
    }
}

impl RecoveryEvidence {
    fn validate(&self, prefix: &str) -> Result<()> {
        ensure!(
            self.tag.starts_with(&format!("{prefix}/")),
            "history recovery tag {} is outside prefix {prefix}",
            self.tag
        );
        for (label, value) in [
            ("recovery tag object", self.tag_object.as_str()),
            ("history old base", self.old_base.as_str()),
            ("history old tip", self.old_tip.as_str()),
        ] {
            ensure!(is_full_sha(value), "{label} must be a full SHA");
        }
        Ok(())
    }
}

pub fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_patch_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_whitespace)
}

fn valid_metadata(value: &str) -> bool {
    !value.trim().is_empty() && value.trim() == value && !value.contains(['\n', '\r'])
}

fn validate_scope(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && !value.starts_with('/') && !value.split('/').any(|part| part == ".."),
        "invalid repository scope: {value}"
    );
    build_glob(value)?;
    Ok(())
}

fn validate_repo_path_text(value: &str) -> Result<()> {
    let path = Path::new(value);
    ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "path must be repository-relative: {value}"
    );
    Ok(())
}

fn validate_repo_path(repo: &Path, value: &str) -> Result<()> {
    validate_repo_path_text(value)?;
    ensure!(
        repo.join(value).starts_with(repo),
        "path escapes repository: {value}"
    );
    Ok(())
}

pub fn scope_matches(pattern: &str, path: &str) -> bool {
    build_glob(pattern).is_ok_and(|glob| glob.compile_matcher().is_match(path))
}

fn build_glob(pattern: &str) -> Result<globset::Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
}

fn normalized_patch_description(name: &str) -> String {
    name.chars()
        .map(|character| match character {
            '-' | '_' => ' ',
            other => other.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn conventional_subject(commit_type: &str, scope: Option<&str>, description: &str) -> String {
    match scope {
        Some(scope) => format!("{commit_type}({scope}): {description}"),
        None => format!("{commit_type}: {description}"),
    }
}

fn valid_commit_token(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_commit_description(value: &str) -> bool {
    valid_metadata(value)
        && value
            .bytes()
            .next()
            .is_some_and(|byte| !byte.is_ascii_uppercase())
        && !value.ends_with('.')
}

fn parse_conventional_prefix(value: &str) -> Result<(&str, Option<&str>), String> {
    if let Some((commit_type, scoped)) = value.split_once('(') {
        let scope = scoped
            .strip_suffix(')')
            .ok_or_else(|| "commit scope must end with `)`".to_string())?;
        if scope.contains(['(', ')']) {
            return Err("commit scope must not contain parentheses".into());
        }
        return Ok((commit_type, Some(scope)));
    }
    if value.contains(')') {
        return Err("commit scope must start with `(`".into());
    }
    Ok((value, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        let commit = "0".repeat(40);
        Manifest {
            schema: 1,
            downstream: Downstream {
                remote: "origin".into(),
                branch: "main".into(),
                recovery_tag_prefix: "forkctl/recovery".into(),
                publish: PublishMode::Rewrite,
            },
            upstream: Upstream {
                remote: "upstream".into(),
                url: "https://example.com/upstream.git".into(),
                fetch_ref: "refs/heads/main".into(),
            },
            base: Base {
                target: BaseTarget {
                    kind: TargetKind::Commit,
                    selector: commit.clone(),
                    commit: commit.clone(),
                    tag_object: None,
                },
                canonical: commit.clone(),
                stack: commit,
            },
            documents: Documents {
                ledger: "PATCHES.md".into(),
                exports: "patches/downstream".into(),
            },
            bookkeeping_patch: "fork-tooling".into(),
            commit_messages: CommitMessagePolicy::default(),
            patches: vec![Patch {
                name: "fork-tooling".into(),
                kind: PatchKind::Tooling,
                purpose: "Own downstream tooling.".into(),
                upstream_status: "downstream-only".into(),
                drop_when: "The fork is retired.".into(),
                commit: None,
                scope: vec![
                    "fork.yaml".into(),
                    "PATCHES.md".into(),
                    "patches/downstream/**".into(),
                ],
                checks: Vec::new(),
            }],
            disabled_patches: Vec::new(),
            history: Vec::new(),
            contracts: Contracts::default(),
        }
    }

    #[test]
    fn accepts_tooling_only_stack() {
        let directory = tempfile::tempdir().unwrap();
        manifest()
            .validate(directory.path(), &directory.path().join("fork.yaml"))
            .unwrap();
    }

    #[test]
    fn source_exports_are_deterministic() {
        let mut value = manifest();
        value.patches.insert(0, source_patch());
        assert_eq!(
            value.source_exports()[0].path,
            "patches/downstream/0001-source.patch"
        );
    }

    #[test]
    fn glob_scope_distinguishes_segments() {
        assert!(scope_matches("src/**", "src/app/mod.rs"));
        assert!(!scope_matches("src/*.rs", "src/app/mod.rs"));
        assert!(scope_matches("src/*.rs", "src/lib.rs"));
    }

    #[test]
    fn rejects_source_after_tooling() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value.patches.push(source_patch());
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.yaml"))
                .is_err()
        );
    }

    fn source_patch() -> Patch {
        Patch {
            name: "source".into(),
            kind: PatchKind::Source,
            purpose: "Change source.".into(),
            upstream_status: "not-submitted".into(),
            drop_when: "Upstream changes.".into(),
            commit: None,
            scope: vec!["src/**".into()],
            checks: Vec::new(),
        }
    }

    #[test]
    fn commit_subjects_default_by_patch_kind() {
        let policy = CommitMessagePolicy::default();
        let source = source_patch();
        let tooling = &manifest().patches[0];

        assert_eq!(source.subject(&policy), "feat: source");
        assert_eq!(tooling.subject(&policy), "chore(tooling): fork tooling");
    }

    #[test]
    fn commit_subject_override_is_complete() {
        let mut patch = source_patch();
        patch.commit =
            Some(PatchCommitMessage::parse("fix(remote): reject crossed sockets").unwrap());

        assert_eq!(
            patch.subject(&CommitMessagePolicy::default()),
            "fix(remote): reject crossed sockets"
        );
    }

    #[test]
    fn commit_conventions_and_descriptions_are_validated() {
        assert!(CommitConvention::parse("feat").is_ok());
        assert!(CommitConvention::parse("chore(tooling)").is_ok());
        assert!(CommitConvention::parse("Feat").is_err());
        assert!(PatchCommitMessage::parse("fix: lowercase description").is_ok());
        assert!(PatchCommitMessage::parse("fix: Uppercase description").is_err());
        assert!(PatchCommitMessage::parse("fix: trailing period.").is_err());
    }
}
