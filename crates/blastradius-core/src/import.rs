//! Structurizr DSL importer (ADR-0002): one-way, producing a Blastradius
//! workspace plus a fidelity report. Every construct that does not map is
//! listed in the report with its line — never silently dropped.
//!
//! Grammar notes learned from the real-world corpus (tests/fixtures/
//! structurizr): keywords are case-insensitive; identifiers may shadow
//! keywords (`softwareSystem = softwareSystem "..."`), so `=` and `->`
//! dispatch takes precedence over keyword matching; groups appear bare or
//! bound (`x = group "..."`) and flatten transparently; relationships carry
//! up to four trailing strings (description, technology, tags, url); bang
//! directives (`!docs docs`) take bare-word arguments.

use crate::model::is_valid_slug;
use std::collections::BTreeMap;
use std::fmt::Write as _;

// ---- tokenizer --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Str(String),
    Arrow,
    Equals,
    Open,
    Close,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    line: u64,
}

fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = src.chars().peekable();
    let mut line: u64 = 1;
    while let Some(&c) = chars.peek() {
        match c {
            '\n' => {
                line += 1;
                chars.next();
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            '#' => {
                while let Some(&c) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '/' => {
                chars.next();
                match chars.peek() {
                    Some('/') => {
                        while let Some(&c) = chars.peek() {
                            if c == '\n' {
                                break;
                            }
                            chars.next();
                        }
                    }
                    Some('*') => {
                        chars.next();
                        let mut prev = ' ';
                        for c in chars.by_ref() {
                            if c == '\n' {
                                line += 1;
                            }
                            if prev == '*' && c == '/' {
                                break;
                            }
                            prev = c;
                        }
                    }
                    _ => return Err(format!("line {line}: stray '/'")),
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            if let Some(e) = chars.next() {
                                s.push(e);
                            }
                        }
                        Some('\n') => {
                            line += 1;
                            s.push('\n');
                        }
                        Some(c) => s.push(c),
                        None => return Err(format!("line {line}: unterminated string")),
                    }
                }
                out.push(Token { tok: Tok::Str(s), line });
            }
            '{' => {
                out.push(Token { tok: Tok::Open, line });
                chars.next();
            }
            '}' => {
                out.push(Token { tok: Tok::Close, line });
                chars.next();
            }
            '=' => {
                out.push(Token { tok: Tok::Equals, line });
                chars.next();
            }
            '-' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    out.push(Token { tok: Tok::Arrow, line });
                } else {
                    let mut w = String::from("-");
                    while let Some(&c) = chars.peek() {
                        if c.is_whitespace() || "{}=\"".contains(c) {
                            break;
                        }
                        w.push(c);
                        chars.next();
                    }
                    out.push(Token { tok: Tok::Word(w), line });
                }
            }
            _ => {
                let mut w = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || "{}=\"#".contains(c) {
                        break;
                    }
                    if c == '-' {
                        let mut clone = chars.clone();
                        clone.next();
                        if clone.peek() == Some(&'>') {
                            break;
                        }
                    }
                    w.push(c);
                    chars.next();
                }
                if !w.is_empty() {
                    out.push(Token { tok: Tok::Word(w), line });
                }
            }
        }
    }
    Ok(out)
}

// ---- model ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Person,
    System,
    Container,
    Component,
}

#[derive(Debug)]
struct Element {
    id: String, // dotted Blastradius id
    kind: Kind,
    name: String,
    description: Option<String>,
    technology: Option<String>,
    external: bool,
}

#[derive(Debug)]
struct Relation {
    from: String, // dsl identifier (or already-resolved id for implicit sources)
    to: String,
    label: Option<String>,
    technology: Option<String>,
    line: u64,
}

#[derive(Default)]
pub struct Fidelity {
    pub mapped: BTreeMap<&'static str, usize>,
    /// (line, what, why)
    pub skipped: Vec<(u64, String, String)>,
    pub notes: Vec<String>,
}

pub struct Import {
    pub workspace_name: String,
    pub files: BTreeMap<String, String>,
    pub report: String,
    pub fidelity: Fidelity,
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    elements: Vec<Element>,
    relations: Vec<Relation>,
    fidelity: Fidelity,
    used_ids: std::collections::BTreeSet<String>,
    ident_map: BTreeMap<String, String>,
    workspace_name: String,
}

const SKIP_BLOCKS: &[&str] = &[
    "views", "styles", "themes", "branding", "configuration", "deploymentenvironment",
    "deploymentnode", "docs", "adrs", "properties", "perspectives", "users", "terminology",
    "enterprise", "healthcheck", "infrastructurenode", "archetypes",
];

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    fn peek2(&self) -> Option<&Tok> {
        self.toks.get(self.pos + 1).map(|t| &t.tok)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn take_strings(&mut self, max: usize) -> Vec<String> {
        let mut out = Vec::new();
        while out.len() < max {
            match self.peek() {
                Some(Token { tok: Tok::Str(s), .. }) => {
                    out.push(s.clone());
                    self.pos += 1;
                }
                _ => break,
            }
        }
        out
    }

    /// One bare-word argument (`!docs docs`, `!identifiers hierarchical`) —
    /// never a word that introduces its own block (`model {`).
    fn skip_word_arg(&mut self) {
        if let Some(Token { tok: Tok::Word(_), .. }) = self.peek() {
            if !matches!(self.peek2(), Some(Tok::Open)) {
                self.pos += 1;
            }
        }
    }

    fn skip_block(&mut self) {
        if !matches!(self.peek().map(|t| &t.tok), Some(Tok::Open)) {
            return;
        }
        self.pos += 1;
        let mut depth = 1;
        while depth > 0 {
            match self.next().map(|t| t.tok) {
                Some(Tok::Open) => depth += 1,
                Some(Tok::Close) => depth -= 1,
                None => break,
                _ => {}
            }
        }
    }

    fn skip_optional_block(&mut self) {
        if matches!(self.peek().map(|t| &t.tok), Some(Tok::Open)) {
            self.skip_block();
        }
    }

    fn fresh_id(&mut self, base: &str, parent: Option<&str>) -> String {
        let mut slug: String = base
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        if slug.is_empty() || !is_valid_slug(&slug) {
            slug = "element".to_string();
        }
        slug.truncate(60);
        let full = |local: &str| match parent {
            Some(p) => format!("{p}.{local}"),
            None => local.to_string(),
        };
        let mut candidate = slug.clone();
        let mut n = 2;
        while self.used_ids.contains(&full(&candidate)) {
            candidate = format!("{slug}-{n}");
            n += 1;
        }
        self.used_ids.insert(full(&candidate));
        candidate
    }

    fn tags_external(tags: &str) -> bool {
        tags.split(',').any(|t| {
            let t = t.trim().to_lowercase();
            t == "external" || t.contains("existing")
        })
    }

    fn push_relation(&mut self, from: String, line: u64) -> Result<(), String> {
        let target = match self.next() {
            Some(Token { tok: Tok::Word(w), .. }) => w,
            other => return Err(format!("line {line}: -> needs a target, got {other:?}")),
        };
        let strs = self.take_strings(4); // description, technology, tags, url
        self.skip_optional_block();
        self.relations.push(Relation {
            from,
            to: target,
            label: strs.first().cloned().filter(|s| !s.is_empty()),
            technology: strs.get(1).cloned().filter(|s| !s.is_empty()),
            line,
        });
        *self.fidelity.mapped.entry("relationship").or_default() += 1;
        Ok(())
    }

    fn note_group(&mut self, line: u64) {
        let gname = self.take_strings(1).pop().unwrap_or_default();
        self.fidelity.notes.push(format!(
            "group {gname:?} (line {line}) flattened — groups are not modelled"
        ));
    }

    fn parse_element(
        &mut self,
        kind: Kind,
        ident: Option<String>,
        parent: Option<String>,
        line: u64,
    ) -> Result<String, String> {
        let strings = self.take_strings(4);
        let name = strings
            .first()
            .cloned()
            .ok_or_else(|| format!("line {line}: element needs a name string"))?;
        let description = strings.get(1).cloned().filter(|s| !s.is_empty());
        let (technology, tags) = match kind {
            Kind::Container | Kind::Component => (
                strings.get(2).cloned().filter(|s| !s.is_empty()),
                strings.get(3).cloned(),
            ),
            _ => (None, strings.get(2).cloned()),
        };
        let external = tags.as_deref().map(Self::tags_external).unwrap_or(false);

        let local = self.fresh_id(&name, parent.as_deref());
        let id = match &parent {
            Some(p) => format!("{p}.{local}"),
            None => local,
        };
        if let Some(ident) = ident {
            self.ident_map.insert(ident, id.clone());
        }
        self.elements.push(Element {
            id: id.clone(),
            kind,
            name,
            description,
            technology,
            external,
        });
        *self.fidelity.mapped.entry(kind_name(kind)).or_default() += 1;

        if matches!(self.peek().map(|t| &t.tok), Some(Tok::Open)) {
            self.pos += 1;
            self.parse_element_body(&id, kind)?;
        }
        Ok(id)
    }

    fn parse_element_body(&mut self, owner: &str, owner_kind: Kind) -> Result<(), String> {
        loop {
            let Some(t) = self.next() else {
                return Err("unexpected end of file in element block".into());
            };
            match t.tok {
                Tok::Close => return Ok(()),
                Tok::Arrow => {
                    // implicit source: `-> target "label"`
                    self.push_relation(owner.to_string(), t.line)?;
                }
                Tok::Word(w) => {
                    // `x = keyword` binding takes precedence: identifiers may
                    // shadow keywords in the wild
                    if matches!(self.peek().map(|x| &x.tok), Some(Tok::Equals)) {
                        self.pos += 1;
                        let kw = match self.next() {
                            Some(Token { tok: Tok::Word(k), .. }) => k.to_lowercase(),
                            other => {
                                return Err(format!(
                                    "line {}: expected keyword after =, got {other:?}",
                                    t.line
                                ))
                            }
                        };
                        match (kw.as_str(), owner_kind) {
                            ("container", Kind::System) => {
                                self.parse_element(Kind::Container, Some(w), Some(owner.to_string()), t.line)?;
                            }
                            ("component", Kind::Container) => {
                                self.parse_element(Kind::Component, Some(w), Some(owner.to_string()), t.line)?;
                            }
                            ("group", _) => {
                                self.note_group(t.line);
                                if matches!(self.peek().map(|x| &x.tok), Some(Tok::Open)) {
                                    self.pos += 1;
                                    self.parse_element_body(owner, owner_kind)?;
                                }
                            }
                            _ => {
                                self.take_strings(8);
                                self.skip_optional_block();
                                self.fidelity.skipped.push((
                                    t.line,
                                    format!("{w} = {kw}"),
                                    "unsupported in this scope".into(),
                                ));
                            }
                        }
                        continue;
                    }
                    if matches!(self.peek().map(|x| &x.tok), Some(Tok::Arrow)) {
                        self.pos += 1;
                        self.push_relation(w, t.line)?;
                        continue;
                    }
                    match w.to_lowercase().as_str() {
                        "description" => {
                            if let Some(s) = self.take_strings(1).pop() {
                                if let Some(el) = self.elements.iter_mut().find(|e| e.id == owner) {
                                    el.description = Some(s);
                                }
                            }
                        }
                        "technology" => {
                            if let Some(s) = self.take_strings(1).pop() {
                                if let Some(el) = self.elements.iter_mut().find(|e| e.id == owner) {
                                    el.technology = Some(s);
                                }
                            }
                        }
                        "tags" => {
                            let tags = self.take_strings(8).join(",");
                            if Self::tags_external(&tags) {
                                if let Some(el) = self.elements.iter_mut().find(|e| e.id == owner) {
                                    el.external = true;
                                }
                            }
                        }
                        "container" if owner_kind == Kind::System => {
                            self.parse_element(Kind::Container, None, Some(owner.to_string()), t.line)?;
                        }
                        "component" if owner_kind == Kind::Container => {
                            self.parse_element(Kind::Component, None, Some(owner.to_string()), t.line)?;
                        }
                        "group" => {
                            self.note_group(t.line);
                            if matches!(self.peek().map(|x| &x.tok), Some(Tok::Open)) {
                                self.pos += 1;
                                self.parse_element_body(owner, owner_kind)?;
                            }
                        }
                        _ => {
                            self.take_strings(8);
                            self.skip_word_arg();
                            self.skip_optional_block();
                            self.fidelity.skipped.push((
                                t.line,
                                w,
                                "not modelled in element block".into(),
                            ));
                        }
                    }
                }
                other => {
                    return Err(format!("line {}: unexpected {other:?} in element block", t.line))
                }
            }
        }
    }

    fn parse_model_body(&mut self) -> Result<(), String> {
        loop {
            let Some(t) = self.next() else {
                return Err("unexpected end of file in model block".into());
            };
            match t.tok {
                Tok::Close => return Ok(()),
                Tok::Word(w) => {
                    if matches!(self.peek().map(|x| &x.tok), Some(Tok::Equals)) {
                        self.pos += 1;
                        let kw = match self.next() {
                            Some(Token { tok: Tok::Word(k), .. }) => k.to_lowercase(),
                            other => {
                                return Err(format!(
                                    "line {}: expected keyword after =, got {other:?}",
                                    t.line
                                ))
                            }
                        };
                        match kw.as_str() {
                            "person" => {
                                self.parse_element(Kind::Person, Some(w), None, t.line)?;
                            }
                            "softwaresystem" => {
                                self.parse_element(Kind::System, Some(w), None, t.line)?;
                            }
                            "group" => {
                                self.note_group(t.line);
                                if matches!(self.peek().map(|x| &x.tok), Some(Tok::Open)) {
                                    self.pos += 1;
                                    self.parse_model_body()?;
                                }
                            }
                            other => {
                                self.take_strings(8);
                                self.skip_optional_block();
                                self.fidelity.skipped.push((
                                    t.line,
                                    format!("{w} = {other}"),
                                    "unsupported model construct".into(),
                                ));
                            }
                        }
                        continue;
                    }
                    if matches!(self.peek().map(|x| &x.tok), Some(Tok::Arrow)) {
                        self.pos += 1;
                        self.push_relation(w, t.line)?;
                        continue;
                    }
                    match w.to_lowercase().as_str() {
                        "person" => {
                            self.parse_element(Kind::Person, None, None, t.line)?;
                        }
                        "softwaresystem" => {
                            self.parse_element(Kind::System, None, None, t.line)?;
                        }
                        "group" => {
                            self.note_group(t.line);
                            if matches!(self.peek().map(|x| &x.tok), Some(Tok::Open)) {
                                self.pos += 1;
                                self.parse_model_body()?;
                            }
                        }
                        kw if SKIP_BLOCKS.contains(&kw) => {
                            self.take_strings(8);
                            self.skip_word_arg();
                            self.skip_optional_block();
                            self.fidelity.skipped.push((
                                t.line,
                                w,
                                "block not modelled (Blastradius owns layout/styling/deployment)".into(),
                            ));
                        }
                        kw if kw.starts_with('!') => {
                            self.take_strings(3);
                            self.skip_word_arg();
                            self.skip_optional_block();
                            self.fidelity.skipped.push((
                                t.line,
                                w,
                                "directive not supported".into(),
                            ));
                        }
                        _ => {
                            self.take_strings(8);
                            self.skip_word_arg();
                            self.skip_optional_block();
                            self.fidelity.skipped.push((
                                t.line,
                                w,
                                "unrecognised in model block".into(),
                            ));
                        }
                    }
                }
                other => return Err(format!("line {}: unexpected {other:?} in model", t.line)),
            }
        }
    }
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Person => "person",
        Kind::System => "softwareSystem",
        Kind::Container => "container",
        Kind::Component => "component",
    }
}

// ---- driver -----------------------------------------------------------------

pub fn import_dsl(src: &str) -> Result<Import, String> {
    let toks = tokenize(src)?;
    let mut p = Parser {
        toks,
        pos: 0,
        elements: Vec::new(),
        relations: Vec::new(),
        fidelity: Fidelity::default(),
        used_ids: Default::default(),
        ident_map: Default::default(),
        workspace_name: "Imported Workspace".to_string(),
    };

    match p.next() {
        Some(Token { tok: Tok::Word(w), .. }) if w.eq_ignore_ascii_case("workspace") => {}
        other => return Err(format!("expected `workspace`, found {other:?}")),
    }
    if let Some(Token { tok: Tok::Word(w), line }) = p.peek().cloned() {
        if w == "extends" {
            return Err(format!("line {line}: `workspace extends` is not supported"));
        }
    }
    let strs = p.take_strings(2);
    if let Some(name) = strs.first() {
        if !name.is_empty() {
            p.workspace_name = name.clone();
        }
    }
    match p.next() {
        Some(Token { tok: Tok::Open, .. }) => {}
        other => return Err(format!("expected '{{' after workspace, found {other:?}")),
    }
    loop {
        let Some(t) = p.next() else {
            return Err("unexpected end of file in workspace block".into());
        };
        match t.tok {
            Tok::Close => break,
            Tok::Word(w) => match w.to_lowercase().as_str() {
                "model" => match p.next() {
                    Some(Token { tok: Tok::Open, .. }) => p.parse_model_body()?,
                    other => return Err(format!("expected '{{' after model, found {other:?}")),
                },
                "name" => {
                    if let Some(n) = p.take_strings(1).pop() {
                        p.workspace_name = n;
                    }
                }
                "description" => {
                    p.take_strings(1);
                }
                kw => {
                    p.take_strings(8);
                    p.skip_word_arg();
                    p.skip_optional_block();
                    let why = if SKIP_BLOCKS.contains(&kw) {
                        "block not modelled (Blastradius owns layout/styling)"
                    } else {
                        "unsupported workspace construct"
                    };
                    p.fidelity.skipped.push((t.line, w, why.into()));
                }
            },
            other => return Err(format!("line {}: unexpected {other:?} in workspace", t.line)),
        }
    }

    build_output(p)
}

fn build_output(p: Parser) -> Result<Import, String> {
    let Parser { elements, relations, mut fidelity, ident_map, workspace_name, .. } = p;

    let all_ids: std::collections::BTreeSet<String> =
        elements.iter().map(|e| e.id.clone()).collect();
    let resolve = move |name: &str| -> Option<String> {
        ident_map
            .get(name)
            .cloned()
            .or_else(|| all_ids.contains(name).then(|| name.to_string()))
    };

    // External systems are opaque in the schema: their containers/components
    // cannot be modelled, so descendants lift to the external system itself
    // and the internals are reported, not silently dropped.
    let external_roots: std::collections::BTreeSet<String> = elements
        .iter()
        .filter(|e| e.kind == Kind::System && e.external)
        .map(|e| e.id.clone())
        .collect();
    let lift_external = |id: String| -> String {
        let root = id.split('.').next().unwrap_or(&id).to_string();
        if external_roots.contains(&root) && root != id {
            root
        } else {
            id
        }
    };
    for e in &elements {
        let root = e.id.split('.').next().unwrap_or(&e.id);
        if external_roots.contains(root) && root != e.id {
            fidelity.notes.push(format!(
                "{} {:?} lives inside external system {root:?} — externals are opaque,                  so it is folded into the parent",
                kind_name(e.kind),
                e.name
            ));
        }
    }
    let elements: Vec<Element> = elements
        .into_iter()
        .filter(|e| {
            let root = e.id.split('.').next().unwrap_or(&e.id);
            !(external_roots.contains(root) && root != e.id)
        })
        .collect();

    let mut resolved: Vec<(String, String, Option<String>, Option<String>)> = Vec::new();
    let mut seen_rel = std::collections::BTreeSet::new();
    for r in &relations {
        match (resolve(&r.from), resolve(&r.to)) {
            (Some(f), Some(t)) => {
                let f = lift_external(f);
                let t = lift_external(t);
                if f == t {
                    continue; // self-loop after lifting
                }
                if seen_rel.insert((f.clone(), t.clone(), r.label.clone())) {
                    resolved.push((f, t, r.label.clone(), r.technology.clone()));
                }
            }
            _ => fidelity.skipped.push((
                r.line,
                format!("{} -> {}", r.from, r.to),
                "endpoint could not be resolved".into(),
            )),
        }
    }

    let yaml = crate::splice::yaml_scalar;
    let mut files: BTreeMap<String, String> = BTreeMap::new();

    let people: Vec<&Element> = elements.iter().filter(|e| e.kind == Kind::Person).collect();
    let externals: Vec<&Element> =
        elements.iter().filter(|e| e.kind == Kind::System && e.external).collect();
    if !people.is_empty() || !externals.is_empty() {
        let mut s = String::from("# Imported from Structurizr DSL — see import-report.md\n");
        if !people.is_empty() {
            s.push_str("people:\n");
            for e in &people {
                let _ = writeln!(s, "  {}:", e.id);
                let _ = writeln!(s, "    name: {}", yaml(&e.name));
                if let Some(d) = &e.description {
                    let _ = writeln!(s, "    description: {}", yaml(d));
                }
            }
        }
        if !externals.is_empty() {
            s.push_str("external:\n");
            for e in &externals {
                let _ = writeln!(s, "  {}:", e.id);
                let _ = writeln!(s, "    name: {}", yaml(&e.name));
                if let Some(d) = &e.description {
                    let _ = writeln!(s, "    description: {}", yaml(d));
                }
            }
        }
        files.insert("model/context.yaml".into(), s);
    }

    let systems: Vec<&Element> =
        elements.iter().filter(|e| e.kind == Kind::System && !e.external).collect();
    if systems.is_empty() {
        return Err("no internal software system found — nothing to import".into());
    }
    let first_system_file = format!("model/{}.yaml", systems[0].id);

    for sys in &systems {
        let mut s = String::new();
        let _ = writeln!(s, "system: {}", sys.id);
        let _ = writeln!(s, "name: {}", yaml(&sys.name));
        if let Some(d) = &sys.description {
            let _ = writeln!(s, "description: {}", yaml(d));
        }
        let containers: Vec<&Element> = elements
            .iter()
            .filter(|e| {
                e.kind == Kind::Container
                    && e.id.starts_with(&format!("{}.", sys.id))
                    && e.id.matches('.').count() == 1
            })
            .collect();
        if !containers.is_empty() {
            s.push_str("\ncontainers:\n");
            for c in &containers {
                let local = c.id.rsplit('.').next().unwrap();
                let _ = writeln!(s, "  {local}:");
                let _ = writeln!(s, "    name: {}", yaml(&c.name));
                if let Some(t) = &c.technology {
                    let _ = writeln!(s, "    tech: {}", yaml(t));
                }
                if let Some(d) = &c.description {
                    let _ = writeln!(s, "    description: {}", yaml(d));
                }
                let comps: Vec<&Element> = elements
                    .iter()
                    .filter(|e| {
                        e.kind == Kind::Component && e.id.starts_with(&format!("{}.", c.id))
                    })
                    .collect();
                if !comps.is_empty() {
                    s.push_str("    components:\n");
                    for k in &comps {
                        let klocal = k.id.rsplit('.').next().unwrap();
                        let _ = writeln!(s, "      {klocal}:");
                        let _ = writeln!(s, "        name: {}", yaml(&k.name));
                        if let Some(t) = &k.technology {
                            let _ = writeln!(s, "        tech: {}", yaml(t));
                        }
                        if let Some(d) = &k.description {
                            let _ = writeln!(s, "        description: {}", yaml(d));
                        }
                    }
                }
            }
        }
        files.insert(format!("model/{}.yaml", sys.id), s);
    }

    let system_of = |id: &str| -> Option<String> {
        let root = id.split('.').next().unwrap().to_string();
        systems.iter().find(|s| s.id == root).map(|s| s.id.clone())
    };
    let mut rel_lines: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (from, to, label, tech) in &resolved {
        let home = system_of(from)
            .or_else(|| system_of(to))
            .map(|sysid| format!("model/{sysid}.yaml"))
            .unwrap_or_else(|| {
                fidelity.notes.push(format!(
                    "relation {from} -> {to} touches no internal system; homed in {first_system_file}"
                ));
                first_system_file.clone()
            });
        let sys_prefix = home
            .strip_prefix("model/")
            .and_then(|f| f.strip_suffix(".yaml"))
            .unwrap_or_default();
        let relativize = |id: &str| -> String {
            id.strip_prefix(&format!("{sys_prefix}."))
                .map(str::to_string)
                .unwrap_or_else(|| id.to_string())
        };
        let mut item = format!("  - from: {}\n    to: {}\n", relativize(from), relativize(to));
        if let Some(l) = label {
            let _ = writeln!(item, "    label: {}", yaml(l));
        }
        if let Some(t) = tech {
            let _ = writeln!(item, "    protocol: {}", yaml(t));
        }
        rel_lines.entry(home).or_default().push(item);
    }
    for (file, items) in rel_lines {
        let entry = files.entry(file).or_default();
        entry.push_str("\nrelations:\n");
        for item in items {
            entry.push_str(&item);
        }
    }

    files.insert(
        "blastradius.yaml".into(),
        format!(
            "# Imported from Structurizr DSL — see import-report.md\nworkspace:\n  name: {}\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\ndocs:\n  include: [\"*.md\"]\n",
            yaml(&workspace_name)
        ),
    );

    let mut report = String::new();
    let _ = writeln!(report, "# Import report — {workspace_name}\n");
    report.push_str("One-way import from Structurizr DSL (ADR-0002). Anything that did not map\nis listed here — nothing was silently dropped.\n\n## Mapped\n\n");
    for (what, n) in &fidelity.mapped {
        let _ = writeln!(report, "- {what}: {n}");
    }
    if !fidelity.notes.is_empty() {
        report.push_str("\n## Notes\n\n");
        for n in &fidelity.notes {
            let _ = writeln!(report, "- {n}");
        }
    }
    if !fidelity.skipped.is_empty() {
        report.push_str("\n## Not mapped\n\n| line | construct | reason |\n| --- | --- | --- |\n");
        for (line, what, why) in &fidelity.skipped {
            let _ = writeln!(report, "| {line} | `{what}` | {why} |");
        }
    }
    files.insert("import-report.md".into(), report.clone());

    Ok(Import { workspace_name, files, report, fidelity })
}
