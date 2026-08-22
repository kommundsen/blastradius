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
}

impl ElementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ElementKind::Person => "person",
            ElementKind::External => "external system",
            ElementKind::System => "system",
            ElementKind::Container => "container",
            ElementKind::Component => "component",
        }
    }
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
    /// Workspace-relative file that declares this element.
    pub file: String,
    pub line: u64,
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
}
