// Blastradius C# L4 extractor (spec/l4-introspection.md, ADR-0016).
//
// Reads the mapping as JSON on stdin ({component, repoRoot, root, include,
// exclude, mode}), emits a facts file on stdout.
//
// Default mode is syntax: Roslyn syntax trees only — no MSBuild, no restore.
// Namespaces and type declarations with name-based edges, resolved against
// the extracted corpus; ambiguous or external names are dropped, not guessed
// (under-reporting beats false edges).
//
// `mode: semantic` (or --semantic) additionally loads the target's own
// solution through MSBuildWorkspace and resolves edges from real symbols,
// which catches what name matching cannot: same-named types in different
// projects, global usings, cross-project references. It is strictly
// best-effort — any failure falls back to the syntax pass with a warning on
// stderr, so semantic mode is never worse than syntax mode. The effective
// mode is recorded in the facts' `extractor` string.
//
// Output is canonical (sorted, 2-space, LF, trailing newline) so the fixture
// test can byte-compare without core's help. File selection and the digest
// protocol mirror core's collect_files/source_digest exactly.

using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

var input = JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;
string component = input.GetProperty("component").GetString()!;
string repoRoot = input.GetProperty("repoRoot").GetString()!;
string root = input.GetProperty("root").GetString()!;
string[] include = OptionalList(input, "include");
string[] exclude = OptionalList(input, "exclude");
bool wantSemantic =
    args.Contains("--semantic") ||
    (input.TryGetProperty("mode", out var modeProp) && modeProp.GetString() == "semantic");

// Resolved once, up front: semantic mode changes the working directory to
// honor the target's global.json, so nothing downstream may depend on cwd.
repoRoot = Path.GetFullPath(repoRoot);
string rootDir = Path.GetFullPath(Path.Combine(repoRoot, root.Replace('/', Path.DirectorySeparatorChar)));
if (!Directory.Exists(rootDir))
{
    Console.Error.WriteLine($"source root {root} does not exist under the repo root");
    return 1;
}

var files = CollectFiles(rootDir, include, exclude);
string digest = SourceDigest(rootDir, files);

// ---- pass 1: namespaces and type declarations (partials merged by FQN) ----

var namespaces = new SortedSet<string>(StringComparer.Ordinal);
var types = new SortedDictionary<string, TypeFact>(StringComparer.Ordinal);
var trees = new List<(string Rel, CompilationUnitSyntax Unit)>();

foreach (var rel in files)
{
    var text = File.ReadAllText(Path.Combine(rootDir, rel));
    var unit = CSharpSyntaxTree.ParseText(text).GetCompilationUnitRoot();
    trees.Add((rel, unit));
    Walk(unit.Members, Ns: "", rel);
}

void Walk(SyntaxList<MemberDeclarationSyntax> members, string Ns, string rel)
{
    foreach (var member in members)
    {
        switch (member)
        {
            case BaseNamespaceDeclarationSyntax ns: // block-scoped and file-scoped
                var full = Ns.Length == 0 ? ns.Name.ToString() : $"{Ns}.{ns.Name}";
                foreach (var part in NamespacePrefixes(full)) namespaces.Add(part);
                Walk(ns.Members, full, rel);
                break;
            case BaseTypeDeclarationSyntax type: // class/struct/record/interface/enum
                RegisterType(type, Ns, rel);
                break;
        }
    }
}

void RegisterType(BaseTypeDeclarationSyntax type, string Ns, string rel)
{
    // Nested types fold into their outermost type (spec) — only register
    // types whose parent is a namespace or the compilation unit.
    if (type.Parent is BaseTypeDeclarationSyntax) return;
    string name = type.Identifier.ValueText;
    string id = Ns.Length == 0 ? name : $"{Ns}.{name}";
    string kind = type switch
    {
        InterfaceDeclarationSyntax => "interface",
        EnumDeclarationSyntax => "enum",
        RecordDeclarationSyntax => "record",
        _ => "class", // class + struct
    };
    int line = type.Identifier.GetLocation().GetLineSpan().StartLinePosition.Line + 1;
    string path = $"{root.TrimEnd('/')}/{rel}";
    if (types.TryGetValue(id, out var existing))
    {
        // Partial declarations merge; the lexicographically first path wins
        // so the result is order-independent.
        if (string.CompareOrdinal(path, existing.Path) < 0)
            types[id] = existing with { Path = path, Line = line };
    }
    else
    {
        types[id] = new TypeFact(id, kind, name, Ns.Length == 0 ? null : Ns, path, line);
    }
}

// Namespaces referenced by types must exist as elements, plus their prefixes
// for nesting (Acme.Billing nests under Acme once both are present).
foreach (var t in types.Values)
    if (t.Parent is not null)
        foreach (var part in NamespacePrefixes(t.Parent))
            namespaces.Add(part);

var byName = types.Values
    .GroupBy(t => t.Name, StringComparer.Ordinal)
    .ToDictionary(g => g.Key, g => g.Select(t => t.Id).ToList(), StringComparer.Ordinal);

// ---- pass 2: name-based edges ---------------------------------------------

var edges = new SortedSet<string>(StringComparer.Ordinal);
var deps = new SortedSet<string>(StringComparer.Ordinal);
// References that leave this mapping's corpus but stay inside the repository,
// as "<element id>\0<repo-relative file>" (ADR-0019). Drift detection resolves
// the file to whichever component owns it, which is a question only the whole
// workspace can answer.
//
// Semantic mode only, and that is the honest limit ADR-0019 already recorded:
// at syntax level C# resolves namespaces rather than paths, so there is no
// file to point at. Semantic mode has the symbol, and a symbol declared in
// source knows the file it was declared in.
var outbound = new SortedSet<string>(StringComparer.Ordinal);
// Absolute paths of the files this mapping covers, so a reference back into
// one of them is never called outbound.
var mappedFiles = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

// Namespace roots this corpus owns; a using outside them is external.
var corpusRoots = new HashSet<string>(
    namespaces.Where(n => !n.Contains('.')), StringComparer.Ordinal);

// Pass 2 as a function: semantic mode replaces the edge set wholesale but
// reuses every element the syntax pass already built.
void SyntaxEdges()
{
    foreach (var (rel, unit) in trees)
    {
        // The file's using set: plain namespace usings widen resolution;
        // alias usings map a name straight to a corpus type.
        var usings = new List<string>();
        var aliases = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var u in unit.DescendantNodes().OfType<UsingDirectiveSyntax>())
        {
            var target = u.Name?.ToString();
            if (target is null) continue;
            if (u.Alias is not null)
            {
                if (types.ContainsKey(target)) aliases[u.Alias.Name.Identifier.ValueText] = target;
            }
            else
            {
                usings.Add(target);
            }
        }

        // Namespaces this file actually declares types in — the elements that
        // own its using directives (below).
        var fileNamespaces = new SortedSet<string>(StringComparer.Ordinal);

        foreach (var type in unit.DescendantNodes().OfType<BaseTypeDeclarationSyntax>())
        {
            if (type.Parent is BaseTypeDeclarationSyntax) continue;
            string ns = EnclosingNamespace(type);
            string selfId = ns.Length == 0 ? type.Identifier.ValueText : $"{ns}.{type.Identifier.ValueText}";
            if (!types.ContainsKey(selfId)) continue;
            if (ns.Length > 0 && namespaces.Contains(ns)) fileNamespaces.Add(ns);

            string? Resolve(string name)
            {
                if (aliases.TryGetValue(name, out var aliased)) return aliased;
                // Same namespace (and enclosing namespace prefixes) first.
                foreach (var prefix in NamespacePrefixes(ns).Reverse().Append(""))
                {
                    var candidate = prefix.Length == 0 ? name : $"{prefix}.{name}";
                    if (types.ContainsKey(candidate)) return candidate;
                }
                foreach (var u in usings)
                    if (types.ContainsKey($"{u}.{name}")) return $"{u}.{name}";
                // Qualified or unique-in-corpus names.
                if (types.ContainsKey(name)) return name;
                var last = name.Contains('.') ? name[(name.LastIndexOf('.') + 1)..] : name;
                return byName.TryGetValue(last, out var ids) && ids.Count == 1 && !name.Contains('.') ? ids[0] : null;
            }

            // Base list: interface targets are implements, the rest extends.
            if (type.BaseList is not null)
            {
                foreach (var b in type.BaseList.Types)
                {
                    var target = Resolve(StripGenerics(b.Type.ToString()));
                    if (target is null || target == selfId) continue;
                    var kind = types[target].Kind == "interface" ? "implements" : "extends";
                    edges.Add($"{selfId} {target} {kind}");
                }
            }

            // Identifier references in the type's body (signatures and bodies).
            foreach (var ident in type.DescendantNodes().OfType<IdentifierNameSyntax>())
            {
                var name = ident.Identifier.ValueText;
                if (name.Length == 0 || !char.IsUpper(name[0])) continue;
                var target = Resolve(name);
                if (target is not null && target != selfId)
                    edges.Add($"{selfId} {target} references");
            }
        }
    }

    // extends/implements subsume the base-list identifier references.
    foreach (var e in edges.Where(e => !e.EndsWith(" references")).ToList())
    {
        var parts = e.Split(' ');
        edges.Remove($"{parts[0]} {parts[1]} references");
    }
}



// Dependency rollups read using directives, not resolved symbols, so they
// are mode-independent and run for both passes (spec).
void DependencyRollups()
{
    foreach (var (_, unit) in trees)
    {
        // Namespaces this file declares types into own its using directives:
        // a using is file-scoped, so per-type edges would invent precision
        // the syntax cannot carry (spec).
        var fileNamespaces = new SortedSet<string>(StringComparer.Ordinal);
        foreach (var type in unit.DescendantNodes().OfType<BaseTypeDeclarationSyntax>())
        {
            if (type.Parent is BaseTypeDeclarationSyntax) continue;
            var ns = EnclosingNamespace(type);
            if (ns.Length > 0 && namespaces.Contains(ns)) fileNamespaces.Add(ns);
        }
        if (fileNamespaces.Count == 0) continue; // nothing to attribute the import to

        foreach (var u in unit.DescendantNodes().OfType<UsingDirectiveSyntax>())
        {
            if (u.Alias is not null) continue;
            var target = u.Name?.ToString();
            if (target is null) continue;
            var nsRoot = target.Contains('.') ? target[..target.IndexOf('.')] : target;
            if (nsRoot.Length == 0 || nsRoot == "System" || corpusRoots.Contains(nsRoot)) continue;
            var id = $"dep.{nsRoot}";
            if (types.ContainsKey(id) || namespaces.Contains(id)) continue; // corpus id wins
            deps.Add(nsRoot);
            foreach (var ns in fileNamespaces) edges.Add($"{ns}\0{id}\0imports");
        }
    }
}

// ---- mode dispatch ---------------------------------------------------------

// Semantic mode is best-effort by contract: any failure degrades to the
// syntax pass rather than producing worse facts (spec). The effective mode
// rides along in the extractor string so committed facts say which ran.
string effectiveMode = "syntax";
if (wantSemantic)
{
    var semantic = TrySemanticEdges(out var why);
    if (semantic is not null)
    {
        foreach (var e in semantic) edges.Add(e);
        effectiveMode = "semantic";
    }
    else
    {
        Console.Error.WriteLine($"semantic mode unavailable ({why}) — falling back to syntax-level");
        SyntaxEdges();
        effectiveMode = "syntax-fallback";
    }
}
else
{
    SyntaxEdges();
}
// Syntax rollups read using directives; semantic mode resolved real symbols
// and named the assemblies instead, so running both would report the same
// dependency twice under two different ids.
if (effectiveMode != "semantic") DependencyRollups();

// Symbol-resolved edges, or null with a reason. Never throws.
SortedSet<string>? TrySemanticEdges(out string why)
{
    why = "";
    try
    {
        // The locator reads global.json from the working directory, so switch
        // to the target repo first: the solution loads under the SDK it pins,
        // not whichever one happens to be first on PATH (spec).
        Directory.SetCurrentDirectory(repoRoot);
        if (!Microsoft.Build.Locator.MSBuildLocator.IsRegistered)
        {
            var instances = Microsoft.Build.Locator.MSBuildLocator.QueryVisualStudioInstances().ToList();
            if (instances.Count == 0) { why = "no MSBuild instance found"; return null; }
            Microsoft.Build.Locator.MSBuildLocator.RegisterInstance(
                instances.OrderByDescending(i => i.Version).First());
        }
        return SemanticEdgesCore(out why);
    }
    catch (Exception e)
    {
        why = e.GetType().Name + ": " + e.Message.Split('\n')[0];
        return null;
    }
}

// Kept out of TrySemanticEdges so MSBuildWorkspace types are not loaded until
// after the locator has registered — the JIT resolves them on entry.
[System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
SortedSet<string>? SemanticEdgesCore(out string why)
{
    why = "";
    var targets = FindProjects(rootDir);
    if (targets.Count == 0) { why = "no .sln or .csproj under the source root"; return null; }

    using var workspace = Microsoft.CodeAnalysis.MSBuild.MSBuildWorkspace.Create();
    workspace.SkipUnrecognizedProjects = true;
    var projects = new List<Project>();
    foreach (var t in targets)
    {
        if (t.EndsWith(".sln", StringComparison.OrdinalIgnoreCase))
            projects.AddRange(workspace.OpenSolutionAsync(t).GetAwaiter().GetResult().Projects);
        else
            projects.Add(workspace.OpenProjectAsync(t).GetAwaiter().GetResult());
    }
    var failures = workspace.Diagnostics
        .Where(d => d.Kind == Microsoft.CodeAnalysis.WorkspaceDiagnosticKind.Failure).ToList();
    if (projects.Count == 0)
    {
        why = failures.Count > 0 ? failures[0].Message.Split('\n')[0] : "no projects loaded";
        return null;
    }

    // Only the files this mapping covers take part; the corpus is still the
    // element set pass 1 built.
    var wanted = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
    foreach (var rel in files) wanted[Path.GetFullPath(Path.Combine(rootDir, rel))] = rel;
    foreach (var full in wanted.Keys) mappedFiles.Add(full);

    var idBySymbol = new Dictionary<ISymbol, string>(SymbolEqualityComparer.Default);
    var models = new List<(SemanticModel Model, SyntaxNode Root)>();
    foreach (var project in projects)
    {
        var compilation = project.GetCompilationAsync().GetAwaiter().GetResult();
        if (compilation is null) continue;
        foreach (var tree in compilation.SyntaxTrees)
        {
            if (tree.FilePath is null || !wanted.ContainsKey(Path.GetFullPath(tree.FilePath))) continue;
            var model = compilation.GetSemanticModel(tree);
            var treeRoot = tree.GetRoot();
            models.Add((model, treeRoot));
            foreach (var decl in treeRoot.DescendantNodes().OfType<BaseTypeDeclarationSyntax>())
            {
                if (model.GetDeclaredSymbol(decl) is not INamedTypeSymbol sym) continue;
                var id = FactIdOf(sym);
                if (id is not null) idBySymbol[sym] = id;
            }
        }
    }
    if (models.Count == 0) { why = "no mapped file belongs to a loaded project"; return null; }

    var result = new SortedSet<string>(StringComparer.Ordinal);
    foreach (var (model, treeRoot) in models)
    {
        foreach (var decl in treeRoot.DescendantNodes().OfType<BaseTypeDeclarationSyntax>())
        {
            if (decl.Parent is BaseTypeDeclarationSyntax) continue;
            if (model.GetDeclaredSymbol(decl) is not INamedTypeSymbol self) continue;
            var selfId = FactIdOf(self);
            if (selfId is null) continue;

            if (decl.BaseList is not null)
            {
                foreach (var b in decl.BaseList.Types)
                {
                    if (model.GetSymbolInfo(b.Type).Symbol is not INamedTypeSymbol target) continue;
                    var id = FactIdOf(target);
                    if (id is null) { NoteOutbound(self, target); AddDependency(result, self, target); continue; }
                    if (id == selfId) continue;
                    result.Add($"{selfId}\0{id}\0{(target.TypeKind == TypeKind.Interface ? "implements" : "extends")}");
                }
            }

            foreach (var node in decl.DescendantNodes().OfType<SimpleNameSyntax>())
            {
                var sym = model.GetSymbolInfo(node).Symbol;
                var named = sym as INamedTypeSymbol ?? sym?.ContainingType;
                if (named is null) continue;
                var id = FactIdOf(named);
                if (id is null) { NoteOutbound(self, named); AddDependency(result, self, named); continue; }
                if (id == selfId) continue;
                result.Add($"{selfId}\0{id}\0references");
            }
        }
    }

    // extends/implements subsume the plain reference, same as syntax mode.
    foreach (var e in result.Where(e => !e.EndsWith("\0references")).ToList())
    {
        var parts = e.Split('\0');
        result.Remove($"{parts[0]}\0{parts[1]}\0references");
    }
    return result;
}

// A symbol's fact id, if this corpus owns it: nested types fold into their
// outermost type, exactly as pass 1 registered them.
// A dependency in semantic mode is a *cross-assembly* reference to something
// outside the corpus, named by the assembly it actually lives in - the thing
// you would add to a project file. Syntax mode can only guess it from a using
// directive's first segment, so Newtonsoft.Json arrives as `dep.Newtonsoft`
// there and as `dep.Newtonsoft.Json` here (spec/l4-introspection.md, recorded
// follow-up). A reference into the *same* assembly is your own code that this
// mapping simply does not cover; calling that a dependency would be a lie.
// A reference out of the corpus that is nonetheless *in the repository*: the
// type is declared in source, in a file this mapping does not cover. That is
// the raw material drift detection needs, and the C# extractor produced none
// of it until 0.10.0 — so the product's central claim, documentation that
// cannot quietly rot, was inert on the stack ADR-0016 named first.
//
// Both shapes count. A sibling project in the same solution is the common one,
// because different components usually are different projects. Same-assembly
// is the other: your own code, in a file this mapping simply does not cover.
//
// A symbol from metadata — the framework, a NuGet package — declares no syntax
// and records nothing here. `dep.<Assembly>` already says that, and it is not
// something a component in this repository could own.
void NoteOutbound(INamedTypeSymbol from, INamedTypeSymbol target)
{
    var fromId = FactIdOf(from);
    if (fromId is null) return;
    foreach (var reference in target.DeclaringSyntaxReferences)
    {
        var path = reference.SyntaxTree?.FilePath;
        if (string.IsNullOrEmpty(path)) continue;
        string full;
        try { full = Path.GetFullPath(path); } catch { continue; }
        if (mappedFiles.Contains(full)) continue;
        var rel = Path.GetRelativePath(repoRoot, full).Replace('\\', '/');
        // Outside the repository is nobody's component.
        if (rel.StartsWith("..", StringComparison.Ordinal) || Path.IsPathRooted(rel)) continue;
        outbound.Add($"{fromId}\0{rel}");
    }
}

void AddDependency(SortedSet<string> into, INamedTypeSymbol from, INamedTypeSymbol target)
{
    var assembly = target.ContainingAssembly?.Name;
    if (string.IsNullOrEmpty(assembly)) return;
    if (SymbolEqualityComparer.Default.Equals(from.ContainingAssembly, target.ContainingAssembly)) return;
    // The framework ships with the runtime and carries no architectural
    // signal - the same exclusion syntax mode makes on the System namespace.
    if (assembly == "mscorlib" || assembly == "netstandard" ||
        assembly == "System" || assembly.StartsWith("System.", StringComparison.Ordinal)) return;
    var id = $"dep.{assembly}";
    if (types.ContainsKey(id) || namespaces.Contains(id)) return; // a corpus id wins
    var fromId = FactIdOf(from);
    if (fromId is null) return;
    deps.Add(assembly);
    into.Add($"{fromId} {id} imports");
}

string? FactIdOf(INamedTypeSymbol symbol)
{
    var outer = symbol;
    while (outer.ContainingType is not null) outer = outer.ContainingType;
    var ns = outer.ContainingNamespace is { IsGlobalNamespace: false } n ? n.ToDisplayString() : "";
    var id = ns.Length == 0 ? outer.Name : $"{ns}.{outer.Name}";
    return types.ContainsKey(id) ? id : null;
}

// One solution if there is exactly one, else every project; sorted so the
// choice never depends on directory order.
List<string> FindProjects(string dir)
{
    var slns = Directory.GetFiles(dir, "*.sln", SearchOption.AllDirectories)
        .Where(f => !IsSkipped(f, dir)).OrderBy(f => f, StringComparer.Ordinal).ToList();
    if (slns.Count > 0)
    {
        if (slns.Count > 1)
            Console.Error.WriteLine($"semantic mode: {slns.Count} solutions under the source root — using {Path.GetFileName(slns[0])}");
        return new List<string> { slns[0] };
    }
    return Directory.GetFiles(dir, "*.csproj", SearchOption.AllDirectories)
        .Where(f => !IsSkipped(f, dir)).OrderBy(f => f, StringComparer.Ordinal).ToList();
}

bool IsSkipped(string file, string dir)
{
    var rel = Path.GetRelativePath(dir, file).Replace('\\', '/');
    return rel.Split('/').Any(seg => seg is "bin" or "obj" or "node_modules" or "packages" || seg.StartsWith("."));
}


// ---- emit ------------------------------------------------------------------

var elements = new List<Dictionary<string, object?>>();
foreach (var ns in namespaces)
{
    var parent = ns.Contains('.') ? ns[..ns.LastIndexOf('.')] : null;
    elements.Add(ElementJson(ns, "namespace", ns.Contains('.') ? ns[(ns.LastIndexOf('.') + 1)..] : ns,
        namespaces.Contains(parent ?? "") ? parent : null, PathOfNamespace(ns), null));
}
foreach (var t in types.Values)
    elements.Add(ElementJson(t.Id, t.Kind, t.Name, t.Parent, t.Path, t.Line));
// External dependencies: one parentless, pathless rollup node each (spec).
foreach (var d in deps)
    elements.Add(ElementJson($"dep.{d}", "dependency", d, null, "", null));
elements.Sort((a, b) => string.CompareOrdinal((string)a["id"]!, (string)b["id"]!));

var facts = new Dictionary<string, object?>
{
    ["schema"] = 1,
    ["language"] = "csharp",
    ["extractor"] = $"blastradius-extract-cs 0.4.0 ({effectiveMode})",
    ["component"] = component,
    ["root"] = root,
    ["sourceDigest"] = digest,
    ["elements"] = elements,
    ["edges"] = edges.Select(e =>
    {
        var p = e.Split(' ');
        return new Dictionary<string, object?> { ["from"] = p[0], ["to"] = p[1], ["kind"] = p[2] };
    }).ToList(),
};
// Omitted when empty, which is every syntax-mode run: the key's absence is
// what facts written before this said, and the fixture gate is byte-exact.
if (outbound.Count > 0)
{
    facts["outbound"] = outbound.Select(o =>
    {
        var p = o.Split('\0');
        return new Dictionary<string, object?> { ["from"] = p[0], ["path"] = p[1] };
    }).ToList();
}

var json = JsonSerializer.Serialize(facts, new JsonSerializerOptions
{
    WriteIndented = true,
    Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
});
// Canonical bytes: LF + trailing newline regardless of platform.
Console.Out.Write(json.Replace("\r\n", "\n") + "\n");
return 0;

// ---- helpers ---------------------------------------------------------------

static string[] OptionalList(JsonElement input, string key) =>
    input.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.Array
        ? v.EnumerateArray().Select(e => e.GetString()!).ToArray()
        : Array.Empty<string>();

static IEnumerable<string> NamespacePrefixes(string ns)
{
    int idx = -1;
    while ((idx = ns.IndexOf('.', idx + 1)) >= 0) yield return ns[..idx];
    yield return ns;
}

static string EnclosingNamespace(SyntaxNode node)
{
    var parts = new List<string>();
    for (var p = node.Parent; p is not null; p = p.Parent)
        if (p is BaseNamespaceDeclarationSyntax ns)
            parts.Insert(0, ns.Name.ToString());
    return string.Join('.', parts);
}

static string StripGenerics(string name)
{
    int lt = name.IndexOf('<');
    return lt < 0 ? name : name[..lt];
}

string? PathOfNamespace(string ns) =>
    types.Values.Where(t => t.Parent == ns || (t.Parent?.StartsWith(ns + ".", StringComparison.Ordinal) ?? false))
        .Select(t => t.Path)
        .OrderBy(p => p, StringComparer.Ordinal)
        .FirstOrDefault();

static Dictionary<string, object?> ElementJson(string id, string kind, string name, string? parent, string? path, int? line)
{
    var d = new Dictionary<string, object?> { ["id"] = id, ["kind"] = kind, ["name"] = name };
    if (parent is not null) d["parent"] = parent;
    d["path"] = path ?? "";
    if (line is not null) d["line"] = line;
    return d;
}

static List<string> CollectFiles(string rootDir, string[] include, string[] exclude)
{
    string[] defaults = { "**/*.cs" };
    var inc = (include.Length > 0 ? include : defaults).Select(GlobToRegex).ToArray();
    var exc = exclude.Select(GlobToRegex).ToArray();
    var skip = new HashSet<string>(StringComparer.Ordinal) { "target", "node_modules", "bin", "obj", "dist", "build", "out", "vendor" };
    var outFiles = new List<string>();
    void WalkDir(string dir)
    {
        foreach (var entry in Directory.EnumerateFileSystemEntries(dir).OrderBy(e => e, StringComparer.Ordinal))
        {
            var name = Path.GetFileName(entry);
            if (Directory.Exists(entry))
            {
                if (!name.StartsWith('.') && !skip.Contains(name)) WalkDir(entry);
                continue;
            }
            var rel = Path.GetRelativePath(rootDir, entry).Replace('\\', '/');
            // Generated files are never part of the model (spec).
            if (rel.EndsWith(".g.cs", StringComparison.Ordinal) || rel.EndsWith(".Designer.cs", StringComparison.Ordinal)) continue;
            if (!inc.Any(r => r.IsMatch(rel))) continue;
            if (exc.Any(r => r.IsMatch(rel))) continue;
            outFiles.Add(rel);
        }
    }
    WalkDir(rootDir);
    outFiles.Sort(StringComparer.Ordinal);
    return outFiles;
}

static Regex GlobToRegex(string glob)
{
    var sb = new StringBuilder("^");
    for (int i = 0; i < glob.Length; i++)
    {
        char c = glob[i];
        if (c == '*')
        {
            if (i + 1 < glob.Length && glob[i + 1] == '*')
            {
                if (i + 2 < glob.Length && glob[i + 2] == '/') { sb.Append("(?:[^/]+/)*"); i += 2; }
                else { sb.Append(".*"); i += 1; }
            }
            else sb.Append("[^/]*");
        }
        else if (c == '?') sb.Append("[^/]");
        else sb.Append(Regex.Escape(c.ToString()));
    }
    sb.Append('$');
    return new Regex(sb.ToString());
}

// Must match core byte-for-byte: sha256 over rel + "\n" + sha256(bytes) + "\n".
static string SourceDigest(string rootDir, List<string> files)
{
    using var outer = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
    foreach (var rel in files)
    {
        outer.AppendData(Encoding.UTF8.GetBytes(rel));
        outer.AppendData("\n"u8.ToArray());
        outer.AppendData(SHA256.HashData(File.ReadAllBytes(Path.Combine(rootDir, rel))));
        outer.AppendData("\n"u8.ToArray());
    }
    return "sha256:" + Convert.ToHexString(outer.GetHashAndReset()).ToLowerInvariant();
}

internal record TypeFact(string Id, string Kind, string Name, string? Parent, string Path, int Line);
