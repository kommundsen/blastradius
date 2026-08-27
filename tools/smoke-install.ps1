# Install-layout smoke (docs/roadmap.md 0.7.0).
#
# Three releases in a row shipped a bug that exists ONLY in an installed
# layout: extractors missing from the package (0.6.0), the scaffold refusing a
# repository that already had a README (0.6.1), and the C# extractor being
# unloadable from WindowsApps (0.6.2). CI built the package every time and
# never ran anything out of it.
#
# This is the missing check. It takes a *finished* CLI — a staged portable
# bundle, or the MSIX's execution alias — and puts it through the flow a new
# user takes on a repository it has never seen:
#
#   1. the binary runs at all
#   2. the extractors it will need shipped with it              (0.6.0)
#   3. `init` on a repository that already has files keeps them (0.6.1)
#   4. the workspace it wrote validates
#   5. TypeScript introspection runs from the install
#   6. C# introspection runs from the install, with an absolute root (0.6.2 —
#      the extractor has to load from a directory it cannot write to, and the
#      root has to survive the trip as a normal Windows path, not a \\?\ one)
#
#   .\tools\smoke-install.ps1 -Cli dist\blastradius-0.7.0-windows-x64\blastradius.exe
#   .\tools\smoke-install.ps1 -Cli blastradius.exe -Installed   # after Add-AppxPackage
#
# -ReadOnly denies write access to the bundle first, which is what makes core
# stage the C# extractor into %LOCALAPPDATA% rather than run it in place. That
# is the 0.6.2 code path, and it is reachable without an MSIX at all.
param(
  [Parameter(Mandatory = $true)][string]$Cli,
  [switch]$Installed,      # the CLI came from a package: no extractors/ beside it to inspect
  [switch]$ReadOnly,       # deny write on the bundle first (an install directory is not writable)
  [switch]$SkipDotnet,     # no .NET runtime on this machine
  [switch]$SkipNode        # no node on this machine
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent

function Step($n, $msg) { Write-Host "`n[$n] $msg" -ForegroundColor Cyan }
function Fail($msg) { throw "SMOKE FAILED: $msg" }
function Run($exe, $arguments) {
  $out = & $exe @arguments 2>&1
  return @{ code = $LASTEXITCODE; text = ($out | Out-String) }
}

$bundle = $null
if (-not $Installed) {
  $Cli = (Resolve-Path $Cli).Path
  $bundle = Split-Path $Cli -Parent
}
$scratch = $null

# ---------------------------------------------------------------- 1. it runs
Step 1 'the binary runs'
$r = Run $Cli @('--help')
if ($r.code -ne 0) { Fail "--help exited $($r.code): $($r.text)" }
if ($r.text -notmatch 'blastradius init') { Fail "--help printed something else: $($r.text)" }

# ------------------------------------------------- 2. the extractors shipped
if (-not $Installed) {
  Step 2 'the extractors shipped beside it'
  # Exactly what core resolves at run time (introspect.rs default_command).
  foreach ($entry in 'dotnet\BlastradiusExtract.dll', 'typescript\extract.mjs') {
    if (-not (Test-Path (Join-Path $bundle "extractors\$entry"))) {
      Fail "extractors/$entry is not in the bundle - this is the 0.6.0 bug"
    }
  }
} else {
  Step 2 'skipped (packaged install: the payload is not ours to inspect)'
}

if ($ReadOnly) {
  Step '2b' 'denying write on the bundle'
  $acl = Get-Acl $bundle
  $deny = New-Object System.Security.AccessControl.FileSystemAccessRule(
    [System.Security.Principal.WindowsIdentity]::GetCurrent().Name,
    'Write', 'ContainerInherit,ObjectInherit', 'None', 'Deny')
  $acl.AddAccessRule($deny)
  Set-Acl -Path $bundle -AclObject $acl
}

try {
  # ------------------------------------ 3. init keeps what is already there
  Step 3 'init on a repository that already has files'
  $scratch = Join-Path ([System.IO.Path]::GetTempPath()) "br-smoke-$PID"
  if (Test-Path $scratch) { Remove-Item $scratch -Recurse -Force }
  New-Item -ItemType Directory $scratch | Out-Null
  Set-Content -Path (Join-Path $scratch 'README.md') -NoNewline -Value @"
# Somebody else's project

Not yours to overwrite.
"@
  $before = (Get-FileHash (Join-Path $scratch 'README.md')).Hash

  Copy-Item (Join-Path $repo 'extractors\dotnet\fixtures\src') (Join-Path $scratch 'src') -Recurse
  Copy-Item (Join-Path $repo 'extractors\typescript\fixtures\src') (Join-Path $scratch 'web') -Recurse
  # `source:` roots are repo-root-relative (ADR-0014), so introspection needs
  # a repository to be relative to. Made explicitly rather than by letting
  # init do it, so this step tests the scaffold and nothing else.
  git -C $scratch init --quiet
  if ($LASTEXITCODE -ne 0) { Fail 'could not git init the scratch repository' }

  # The TypeScript extractor uses the compiler API from the repository being
  # analysed, by design (spec/l4-introspection.md) — the bundle ships the
  # extractor, not a copy of typescript. Borrow this repo's, rather than
  # running npm install into a temp directory on every CI run.
  $tsc = Join-Path $repo 'node_modules\typescript'
  if (Test-Path $tsc) {
    New-Item -ItemType Junction -Path (Join-Path $scratch 'node_modules') `
      -Target (Join-Path $repo 'node_modules') | Out-Null
  } elseif (-not $SkipNode) {
    Fail 'node_modules/typescript is missing - run npm ci, or pass -SkipNode'
  }

  $r = Run $Cli @('init', $scratch, '--into', 'docs', '--name', 'Smoke', '--no-git', '--agents', '', '--skills', '')
  if ($r.code -ne 0) { Fail "init exited $($r.code): $($r.text)" }
  $after = (Get-FileHash (Join-Path $scratch 'README.md')).Hash
  if ($before -ne $after) { Fail 'init rewrote an existing README - this is the 0.6.1 bug' }
  $ws = Join-Path $scratch 'docs'
  if (-not (Test-Path (Join-Path $ws 'blastradius.yaml'))) { Fail 'init wrote no workspace' }

  # ------------------------------------------------ 4. what it wrote is valid
  Step 4 'the workspace it wrote validates'
  $r = Run $Cli @('validate', $ws)
  if ($r.code -ne 0) { Fail "validate exited $($r.code): $($r.text)" }

  # --------------------------------------- 5/6. introspection from the install
  # Two source-mapped components over the corpora copied in above. Written
  # rather than scaffolded, because the point is the extractors — and the
  # dogfood workspace has no C# mapping at all, which is exactly why 0.6.2's
  # bug survived three releases.
  Set-Content -Path (Join-Path $ws 'model\smoke.yaml') -Value @"
system: smoke
name: Smoke
containers:
  backend:
    name: Backend
    tech: .NET
    components:
      billing:
        name: Billing
        source:
          language: csharp
          root: src
  web:
    name: Web
    tech: TypeScript
    components:
      store:
        name: Store
        source:
          language: typescript
          root: web
"@
  # The starter model is a different system with its own views; drop both so
  # what is left is unambiguous.
  Remove-Item (Join-Path $ws 'model\context.yaml') -ErrorAction SilentlyContinue
  Remove-Item (Join-Path $ws 'model\help.yaml') -ErrorAction SilentlyContinue
  Remove-Item (Join-Path $ws 'views\*') -Force -ErrorAction SilentlyContinue
  $r = Run $Cli @('validate', $ws)
  if ($r.code -ne 0) { Fail "the smoke workspace does not validate: $($r.text)" }

  if (-not $SkipNode) {
    Step 5 'TypeScript introspection, from the installed extractor'
    $r = Run $Cli @('introspect', $ws, 'smoke.web.store')
    if ($r.code -ne 0) { Fail "typescript introspect exited $($r.code): $($r.text)" }
    $facts = Join-Path $ws 'model\derived\smoke.web.store.l4.json'
    if (-not (Test-Path $facts)) { Fail "typescript introspect wrote no facts: $($r.text)" }
  }

  if (-not $SkipDotnet) {
    Step 6 'C# introspection, from the installed extractor, with an absolute root'
    $r = Run $Cli @('introspect', $ws, 'smoke.backend.billing')
    if ($r.code -ne 0) { Fail "csharp introspect exited $($r.code): $($r.text)" }
    $facts = Join-Path $ws 'model\derived\smoke.backend.billing.l4.json'
    if (-not (Test-Path $facts)) { Fail "csharp introspect wrote no facts: $($r.text)" }
    $json = Get-Content $facts -Raw | ConvertFrom-Json
    if (-not $json.elements) { Fail 'csharp facts are empty' }
    Write-Host "    $($json.elements.Count) elements, $($json.edges.Count) edges"

    if ($ReadOnly) {
      # The whole point of the read-only run: an unwritable install directory
      # must make core stage the extractor into %LOCALAPPDATA% and run it from
      # there. WindowsApps permits reading the DLL but not loading it as an
      # assembly, which is what took C# introspection down in 0.6.2.
      $staged = Join-Path $env:LOCALAPPDATA 'Blastradius'
      if (-not (Test-Path (Join-Path $staged '*\dotnet\BlastradiusExtract.dll'))) {
        Fail 'the extractor was not staged out of the unwritable bundle - the 0.6.2 path did not run'
      }
      Write-Host '    extractor staged out of the read-only bundle, as it must be'
    }
  }

  Write-Host "`nSMOKE PASSED" -ForegroundColor Green
}
finally {
  if ($ReadOnly -and $bundle) {
    $acl = Get-Acl $bundle
    foreach ($rule in @($acl.Access | Where-Object { $_.AccessControlType -eq 'Deny' })) {
      $acl.RemoveAccessRule($rule) | Out-Null
    }
    Set-Acl -Path $bundle -AclObject $acl
  }
  if ($scratch -and (Test-Path $scratch)) {
    Remove-Item $scratch -Recurse -Force -ErrorAction SilentlyContinue
  }
}
