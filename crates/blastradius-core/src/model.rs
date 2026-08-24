//! The in-memory model: elements, relations, views, documents.
//!
//! Identity per ADR-0003: an element's id is its YAML key (immutable slug);
//! the global address is the dotted path. `ElementId` is always the full
//! dotted path; context elements (people/external) are bare ids.

use std::collections::BTreeMap;

pub type ElementId = String;

pub fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    Person,
    External,
    System,
    Container,
    Component,
    /// Deployment (ADR-0018): the physical side of the model. Ordinary
    /// elements — dotted ids, relations, pins, editing — not a parallel
    /// namespace like derived L4 code.
    Environment,
    DeploymentNode,
    ContainerInstance,
}

impl ElementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ElementKind::Person => "person",
            ElementKind::External => "external system",
            ElementKind::System => "system",
            ElementKind::Container => "container",
            ElementKind::Component => "component",
            ElementKind::Environment => "environment",
            ElementKind::DeploymentNode => "deployment node",
            ElementKind::ContainerInstance => "container instance",
        }
    }

    /// True for the deployment kinds — they live in `deployment.yaml` and
    /// render in LD views rather than the L1–L3 altitudes.
    pub fn is_deployment(self) -> bool {
        matches!(self, ElementKind::Environment | ElementKind::DeploymentNode | ElementKind::ContainerInstance)
    }
}

/// Key chain addressing a deployment element in `deployment.yaml`
/// (ADR-0018). Unlike containers and components, deployment nodes nest
/// arbitrarily, so the chain is built from the id rather than fixed per
/// kind — but only the last segment can be an instance, so the element's
/// own kind is enough and no tree walk is needed.
pub fn deployment_chain(id: &str, kind: ElementKind) -> Vec<String> {
    let segs: Vec<&str> = id.split('.').collect();
    let mut chain = vec!["environments".to_string(), segs[0].to_string()];
    for (i, seg) in segs.iter().enumerate().skip(1) {
        let last = i == segs.len() - 1;
        let section = if last && kind == ElementKind::ContainerInstance { "instances" } else { "nodes" };
        chain.push(section.to_string());
        chain.push((*seg).to_string());
    }
    chain
}

#[derive(Debug, Clone)]
pub struct Element {
    pub id: ElementId,
    pub kind: ElementKind,
    pub name: String,
    pub tech: Option<String>,
    pub description: Option<String>,
    /// `external: true` on a system (spec §3).
    pub external: bool,
    /// `source:` mapping — components only (spec/l4-introspection.md).
    pub source: Option<SourceMapping>,
    /// The container a `container-instance` instantiates, as written
    /// (ADR-0018). Resolved and checked in cross-validation.
    pub instance_of: Option<String>,
    /// Workspace-relative file that declares this element.
    pub file: String,
    pub line: u64,
}

/// A component's opt-in to L4 introspection (ADR-0016). Paths are
/// repo-root-relative; globs are extractor-evaluated.
#[derive(Debug, Clone)]
pub struct SourceMapping {
    pub language: String,
    pub root: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// Override for the extractor command; None = the language default.
    pub extractor: Option<String>,
    /// Extraction depth: `syntax` (default) or `semantic`. Only the C#
    /// extractor acts on it (spec/l4-introspection.md).
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Both,
    None,
}

#[derive(Debug, Clone)]
pub struct Relation {
    /// Resolved dotted ids (resolution happens at parse time; unresolved
    /// references are diagnostics, and the relation keeps the raw text so the
    /// diff stays stable).
    pub from: ElementId,
    pub to: ElementId,
    pub label: Option<String>,
    pub protocol: Option<String>,
    pub direction: Direction,
    pub file: String,
    pub line: u64,
    /// System scope the relation was written in — used by validate for
    /// sibling resolution (spec §3), never exposed beyond the crate.
    pub(crate) scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct View {
    pub id: String,
    pub name: Option<String>,
    pub scope: ElementId,
    pub level: String, // validated L1 | L2 | L3
    /// element id -> [x, y] grid units
    pub layout: BTreeMap<ElementId, (f64, f64)>,
    pub include_context: bool,
    pub file: String,
    pub line: u64,
}

#[derive(Debug, Clone)]
pub struct Doc {
    pub id: String,
    pub doc_type: String,
    pub status: Option<String>,
    pub elements: Vec<ElementId>,
    pub supersedes: Option<String>,
    pub file: String,
    pub line: u64,
}

/// Source-derived L4 elements for one opted-in component, loaded from a
/// committed facts file (spec/l4-introspection.md). Kept apart from
/// `Workspace::elements` by design: derived elements never participate in
/// sync, diff, or authored-element validation, and the conformance counts
/// stay authored-only (ADR-0016).
#[derive(Debug, Clone)]
pub struct DerivedGraph {
    /// Owning component's dotted id.
    pub component: ElementId,
    pub language: String,
    /// The facts file's sourceDigest — kept for the staleness probe.
    pub source_digest: String,
    /// Facts digest disagrees with the working tree (badge, not error).
    pub stale: bool,
    pub elements: Vec<DerivedElement>,
    pub edges: Vec<DerivedEdge>,
}

#[derive(Debug, Clone)]
pub struct DerivedElement {
    /// Full dotted id: `<component>.src.<fact-id>`.
    pub id: ElementId,
    /// module | namespace | class | interface | record | enum
    pub kind: String,
    pub name: String,
    /// Full dotted id of the containing module/namespace, if any.
    pub parent: Option<ElementId>,
    /// Repo-root-relative source path (forward slashes).
    pub path: String,
    /// 1-based declaration line for click-through (types only).
    pub line: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DerivedEdge {
    pub from: ElementId,
    pub to: ElementId,
    /// imports | references | extends | implements
    pub kind: String,
}

/// A fully loaded workspace. `elements` is keyed by dotted id and ordered
/// (BTreeMap) so that iteration — and therefore diffing and output — is
/// deterministic (ADR-0006 applies the same principle to layout).
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    pub name: String,
    pub elements: BTreeMap<ElementId, Element>,
    pub relations: Vec<Relation>,
    pub views: Vec<View>,
    pub docs: Vec<Doc>,
    /// One entry per facts file under `model/derived/`, sorted by component.
    pub derived: Vec<DerivedGraph>,
}

impl Workspace {
    /// Resolve a reference as written in a system file (spec §3): bare context
    /// id, sibling path relative to `system`, or absolute dotted path.
    pub fn resolve(&self, reference: &str, system: Option<&str>) -> Option<ElementId> {
        if self.elements.contains_key(reference) {
            return Some(reference.to_string());
        }
        if let Some(sys) = system {
            let scoped = format!("{sys}.{reference}");
            if self.elements.contains_key(&scoped) {
                return Some(scoped);
            }
        }
        None
    }

    /// Look up a derived (introspected) element by full dotted id.
    pub fn derived_element(&self, id: &str) -> Option<&DerivedElement> {
        self.derived.iter().flat_map(|g| g.elements.iter()).find(|e| e.id == id)
    }

    /// Relations with both endpoints resolved to dotted ids (spec §3 sibling
    /// resolution). Unresolvable relations — already reported as diagnostics —
    /// are skipped.
    pub fn resolved_relations(&self) -> Vec<(ElementId, ElementId, &Relation)> {
        self.relations
            .iter()
            .filter_map(|r| {
                let scope = r.scope.as_deref();
                Some((self.resolve(&r.from, scope)?, self.resolve(&r.to, scope)?, r))
            })
            .collect()
    }
}
