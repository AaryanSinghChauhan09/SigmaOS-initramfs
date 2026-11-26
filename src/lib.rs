// SPDX-License-Identifier: Apache-2.0
//! SigmaOS-initramfs — SigmaOS initramfs component inspired by Linux Mint / Omarchy / Arch Linux.
//!
//! Ordered execution plan: validated steps executed fail-fast, first error aborts.
//! Principles: single responsibility, fail-fast, defensive programming,
//! component isolation, stable API, deterministic tests.
#![forbid(unsafe_code)]

pub mod cpio;
pub mod pack;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
/// Errors for `PlanError` operations. Fail-fast: invalid input is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Plan step description must not be empty
    EmptyStep,
    /// Cannot run an empty plan
    EmptyPlan,
    /// A plan step failed during execution
    StepFailed(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStep => write!(f, "plan step description must not be empty"),
            Self::EmptyPlan => write!(f, "cannot run an empty plan"),
            Self::StepFailed(x) => write!(f, "a plan step failed during execution: {x}"),
        }
    }
}

impl std::error::Error for PlanError {}

/// An ordered list of steps forming an executable plan.
#[derive(Debug, Default)]
pub struct Plan {
    steps: Vec<String>,
}

impl Plan {
    /// Empty plan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a step; fails on empty description.
    pub fn add_step(&mut self, step: impl Into<String>) -> Result<(), PlanError> {
        let step = step.into();
        if step.trim().is_empty() {
            return Err(PlanError::EmptyStep);
        }
        self.steps.push(step);
        Ok(())
    }

    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the plan has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Validate the plan is non-empty.
    pub fn validate(&self) -> Result<(), PlanError> {
        if self.steps.is_empty() {
            Err(PlanError::EmptyPlan)
        } else {
            Ok(())
        }
    }

    /// Execute steps in order via `exec`; first failure aborts and is
    /// wrapped in `StepFailed`. Returns the list of completed step names.
    pub fn run<F>(&self, mut exec: F) -> Result<Vec<String>, PlanError>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        self.validate()?;
        let mut done = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            exec(step).map_err(|e| PlanError::StepFailed(format!("{step}: {e}")))?;
            done.push(step.clone());
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        let mut p = Plan::new();
        p.add_step("partition").unwrap();
        p.add_step("install base").unwrap();
        p
    }

    #[test]
    fn run_executes_in_order() {
        let p = plan();
        let done = p.run(|_| Ok(())).unwrap();
        assert_eq!(
            done,
            vec!["partition".to_string(), "install base".to_string()]
        );
    }

    #[test]
    fn failure_aborts_plan() {
        let p = plan();
        let res = p.run(|s| {
            if s == "install base" {
                Err("boom".into())
            } else {
                Ok(())
            }
        });
        assert!(matches!(res, Err(PlanError::StepFailed(_))));
    }

    #[test]
    fn empty_step_rejected() {
        assert!(matches!(
            Plan::new().add_step(" "),
            Err(PlanError::EmptyStep)
        ));
    }

    #[test]
    fn empty_plan_run_fails() {
        assert_eq!(Plan::new().run(|_| Ok(())), Err(PlanError::EmptyPlan));
    }
}

/// Backwards-compatible component handle (kept for existing callers/tests).
#[derive(Debug, Default)]
pub struct Component {
    name: &'static str,
}

impl Component {
    /// Create the component handle.
    pub fn new() -> Self {
        Self {
            name: "SigmaOS Component",
        }
    }

    /// Component label.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// Errors produced by this component.
#[derive(Debug, thiserror::Error)]
pub enum InitramfsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hook error: {0}")]
    Hook(String),
    #[error("cycle detected in hook dependencies: {0}")]
    Cycle(String),
    #[error("hook not found: {0}")]
    NotFound(String),
}

/// What a hook installs into the initramfs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookPayload {
    /// Kernel modules to include (e.g. "nvme", "xhci_hcd").
    #[serde(default)]
    pub modules: Vec<String>,
    /// Binaries to copy (resolved via PATH at build time).
    #[serde(default)]
    pub binaries: Vec<String>,
    /// Config files to embed.
    #[serde(default)]
    pub files: Vec<PathBuf>,
    /// Extra directories to include recursively.
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
}

/// A single initramfs hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    /// Hook name (unique).
    pub name: String,
    /// Hooks that must run before this one.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Relative install order hint (lower runs first within deps).
    #[serde(default)]
    pub order: i32,
    /// What this hook installs.
    #[serde(default)]
    pub payload: HookPayload,
}

/// The initramfs build configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitramfsConfig {
    /// Output image path.
    #[serde(default = "default_image")]
    pub image: PathBuf,
    /// Compression format: "gzip", "zstd", "xz", "none".
    #[serde(default = "default_compression")]
    pub compression: String,
    /// Kernel version tag embedded in the image name.
    #[serde(default)]
    pub kernel: Option<String>,
    /// Hooks to run, in declaration order.
    #[serde(default)]
    pub hooks: Vec<Hook>,
}

fn default_image() -> PathBuf {
    PathBuf::from("/boot/initramfs.img")
}
fn default_compression() -> String {
    "zstd".into()
}

/// A resolved build step.
#[derive(Debug, Clone, Serialize)]
pub struct BuildStep {
    pub hook: String,
    pub modules: Vec<String>,
    pub binaries: Vec<String>,
    pub files: Vec<PathBuf>,
    pub dirs: Vec<PathBuf>,
}

/// Full build plan after dependency resolution.
#[derive(Debug, Clone, Serialize)]
pub struct BuildPlan {
    pub image: PathBuf,
    pub compression: String,
    pub kernel: Option<String>,
    pub steps: Vec<BuildStep>,
}

impl InitramfsConfig {
    /// Load config from TOML.
    pub fn load(path: &Path) -> Result<Self, InitramfsError> {
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Resolve hook order via topological sort (Kahn's algorithm),
    /// tie-broken by `order` then name for determinism.
    pub fn plan(&self) -> Result<BuildPlan, InitramfsError> {
        let names: BTreeSet<&str> = self.hooks.iter().map(|h| h.name.as_str()).collect();
        for h in &self.hooks {
            for d in &h.depends {
                if !names.contains(d.as_str()) {
                    return Err(InitramfsError::NotFound(format!(
                        "hook '{}' depends on missing '{}'",
                        h.name, d
                    )));
                }
            }
        }

        // indegree + adjacency
        let mut indeg: BTreeMap<&str, usize> = BTreeMap::new();
        let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for h in &self.hooks {
            indeg.entry(h.name.as_str()).or_insert(0);
            for d in &h.depends {
                adj.entry(d.as_str()).or_default().push(h.name.as_str());
                *indeg.entry(h.name.as_str()).or_insert(0) += 1;
            }
        }

        // ready set ordered by (order, name)
        let mut ready: Vec<&Hook> = self
            .hooks
            .iter()
            .filter(|h| indeg[h.name.as_str()] == 0)
            .collect();
        ready.sort_by_key(|h| (h.order, h.name.clone()));

        let mut steps = Vec::new();
        let mut done: BTreeSet<&str> = BTreeSet::new();
        while let Some(h) = ready.first().copied() {
            ready.remove(0);
            if !done.insert(h.name.as_str()) {
                continue;
            }
            steps.push(BuildStep {
                hook: h.name.clone(),
                modules: h.payload.modules.clone(),
                binaries: h.payload.binaries.clone(),
                files: h.payload.files.clone(),
                dirs: h.payload.dirs.clone(),
            });
            if let Some(children) = adj.get(h.name.as_str()) {
                for c in children {
                    let e = indeg.get_mut(c).unwrap();
                    *e -= 1;
                    if *e == 0 {
                        if let Some(hook) = self.hooks.iter().find(|x| x.name == *c) {
                            let pos = ready
                                .iter()
                                .position(|r| {
                                    (r.order, r.name.clone()) > (hook.order, hook.name.clone())
                                })
                                .unwrap_or(ready.len());
                            ready.insert(pos, hook);
                        }
                    }
                }
            }
        }

        if steps.len() != self.hooks.len() {
            let remaining: Vec<&str> = self
                .hooks
                .iter()
                .map(|h| h.name.as_str())
                .filter(|n| !done.contains(n))
                .collect();
            return Err(InitramfsError::Cycle(remaining.join(", ")));
        }

        Ok(BuildPlan {
            image: self.image.clone(),
            compression: self.compression.clone(),
            kernel: self.kernel.clone(),
            steps,
        })
    }

    /// Flatten the plan into the list of modules/binaries/files the
    /// image will contain.
    pub fn manifest(&self) -> Result<BuildPlan, InitramfsError> {
        self.plan()
    }
}

/// Serialize a plan to JSON.
pub fn plan_json(plan: &BuildPlan) -> Result<String, InitramfsError> {
    Ok(serde_json::to_string_pretty(plan)?)
}

#[cfg(test)]
mod hook_pipeline_tests {
    use super::*;

    fn hook(name: &str, deps: &[&str], order: i32) -> Hook {
        Hook {
            name: name.into(),
            depends: deps.iter().map(|s| s.to_string()).collect(),
            order,
            payload: HookPayload::default(),
        }
    }

    #[test]
    fn topo_orders_by_dependency() {
        let cfg = InitramfsConfig {
            image: PathBuf::from("/tmp/i.img"),
            compression: "zstd".into(),
            kernel: None,
            hooks: vec![
                hook("b", &["a"], 0),
                hook("a", &[], 0),
                hook("c", &["b"], 0),
            ],
        };
        let plan = cfg.plan().unwrap();
        let names: Vec<&str> = plan.steps.iter().map(|s| s.hook.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn detects_cycle() {
        let cfg = InitramfsConfig {
            image: PathBuf::from("/tmp/i.img"),
            compression: "zstd".into(),
            kernel: None,
            hooks: vec![hook("a", &["b"], 0), hook("b", &["a"], 0)],
        };
        assert!(matches!(cfg.plan(), Err(InitramfsError::Cycle(_))));
    }

    #[test]
    fn missing_dep_errors() {
        let cfg = InitramfsConfig {
            image: PathBuf::from("/tmp/i.img"),
            compression: "zstd".into(),
            kernel: None,
            hooks: vec![hook("a", &["ghost"], 0)],
        };
        assert!(matches!(cfg.plan(), Err(InitramfsError::NotFound(_))));
    }
}
