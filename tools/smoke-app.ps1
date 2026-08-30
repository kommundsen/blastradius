# Install-layout smoke for the *app* (docs/roadmap.md 0.10.0 item 3).
#
# tools/smoke-install.ps1 does this for the CLI, and it is why 0.7.0 stopped
# the run of install-only bugs. The app has never had one. Every release that
# shipped app-side features without an installed run shipped a bug only that
# layout could show (0.6.0, 0.6.1, 0.6.2), and the e2e suite cannot see them:
# it runs against the mock bridge (ADR-0011), which is blind to the whole IPC
# boundary by construction.
#
# So this stages the shipping layout — both binaries and the out-of-process
# extractors beside them, exactly as the portable bundle and the MSIX do —
# makes a throwaway repository that has never seen Blastradius, and launches
# the app *at that repository* with WebView2's remote debugging port open.
# tools/drive-app.mjs then attaches over CDP and walks a new user's flow:
#
#   1. the window opens on the offer, naming the folder                 (0.6.1)
#   2. taking it scaffolds, sets up the agents, and hands over
#   3. a model renders — out of an install, which nothing has ever checked
#   4. an edit goes through the real sync engine onto real YAML
#   5. `introspect_component` runs the staged TypeScript extractor  (0.6.0/0.6.2)
#   6. the derived code is reachable on the canvas
#
# and this script checks what is left on disk afterwards.
#
#   .\tools\smoke-app.ps1                       # builds release, stages, runs
#   .\tools\smoke-app.ps1 -Bundle dist\blastradius-0.10.0-windows-x64
#   .\tools\smoke-app.ps1 -ReadOnly             # deny write on the bundle (0.6.2)
#
# Windows only: WebView2 is the engine with a debugging port. WebKitGTK has no
# equivalent, so Linux and macOS keep the compile check and the mock suite.
param(
  [string]$Bundle,          # a staged bundle to use as-is; default builds one
  [switch]$ReadOnly,        # make the bundle read-only, the way an install is
  [switch]$KeepScratch,     # leave the throwaway repo behind for inspection
  [int]$Port = 9222
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent

function Step($n, $msg) { Write-Host "`n[$n] $msg" -ForegroundColor Cyan }
function Fail($msg) { throw "APP SMOKE FAILED: $msg" }

$proc = $null
$scratch = $null
$acl = $null
$staged = $null

try {
  # ------------------------------------------------------- 1. the install layout
  Step 1 'stage the shipping layout'
  if ($Bundle) {
    $staged = (Resolve-Path $Bundle).Path
  } else {
    $staged = Join-Path $repo 'target/app-smoke/bundle'
    Remove-Item -Recurse -Force $staged -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $staged | Out-Null
    foreach ($bin in 'blastradius.exe', 'blastradius-app.exe') {
      $from = Join-Path $repo "target/release/$bin"
      if (-not (Test-Path $from)) { Fail "missing build output: $from (cargo build --release)" }
      Copy-Item $from (Join-Path $staged $bin)
    }
    # The extractors core looks for beside the running binary. Without them,
    # TypeScript and C# introspection fail on a machine with no checkout —
    # which is what 0.6.0 shipped.
    & node (Join-Path $repo 'tools/stage-extractors.mjs') --out $staged
    if ($LASTEXITCODE -ne 0) { Fail 'staging the extractors failed' }
  }
  $appExe = Join-Path $staged 'blastradius-app.exe'
  if (-not (Test-Path $appExe)) { Fail "no blastradius-app.exe in $staged" }
  Write-Host "    $staged"

  if ($ReadOnly) {
    # An install directory cannot be written to. 0.6.2 shipped a C# extractor
    # that could not load from one.
    $acl = Get-Acl $staged
    $deny = New-Object System.Security.AccessControl.FileSystemAccessRule(
      [System.Security.Principal.WindowsIdentity]::GetCurrent().Name,
      'Write', 'ContainerInherit,ObjectInherit', 'None', 'Deny')
    $ro = Get-Acl $staged
    $ro.AddAccessRule($deny)
    Set-Acl $staged $ro
    Write-Host '    write denied on the bundle'
  }

  # ------------------------------------------------ 2. a repository it has never seen
  Step 2 'make a throwaway repository'
  $scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("br-app-smoke-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
  New-Item -ItemType Directory -Force $scratch | Out-Null
  New-Item -ItemType Directory -Force (Join-Path $scratch 'src') | Out-Null

  # A README the user already had. 0.6.1 treated one as fatal; it must come
  # back byte-identical.
  $readme = "# Throwaway`n`nThis file was here first.`n"
  [System.IO.File]::WriteAllText((Join-Path $scratch 'README.md'), $readme)
  $readmeBefore = [System.IO.File]::ReadAllBytes((Join-Path $scratch 'README.md'))

  # Something for the TypeScript extractor to find.
  [System.IO.File]::WriteAllText((Join-Path $scratch 'src/service.ts'), @'
export interface Order { id: string; total: number }

export class OrderService {
  place(order: Order): void {}
}

export function summarise(orders: Order[]): number {
  return orders.reduce((n, o) => n + o.total, 0);
}
'@)
  & git -C $scratch init --quiet 2>&1 | Out-Null
  Write-Host "    $scratch"

  # ------------------------------------------------------------ 3. launch it
  #
  # Reported before launching, because "no port appeared" has at least three
  # causes and the first run of this in CI could not tell them apart: the app
  # exiting immediately, no WebView2 runtime on the machine, and no desktop
  # session to put a window in. Each leaves different evidence, and none of it
  # was being collected.
  Step 3 "launch the app at the repository (CDP on $Port)"
  $rt = @(
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
  ) | ForEach-Object { try { (Get-ItemProperty $_ -ErrorAction Stop).pv } catch { } } | Select-Object -First 1
  Write-Host "    WebView2 runtime: $(if ($rt) { $rt } else { 'NOT REGISTERED' })"
  Write-Host "    session $([System.Diagnostics.Process]::GetCurrentProcess().SessionId), interactive=$([Environment]::UserInteractive)"

  $outLog = Join-Path ([System.IO.Path]::GetTempPath()) 'br-app-smoke.out'
  $errLog = Join-Path ([System.IO.Path]::GetTempPath()) 'br-app-smoke.err'
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
  $proc = Start-Process -FilePath $appExe -ArgumentList $scratch -PassThru `
    -RedirectStandardOutput $outLog -RedirectStandardError $errLog
  Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS

  $deadline = (Get-Date).AddSeconds(60)
  $up = $false
  while ((Get-Date) -lt $deadline) {
    try {
      Invoke-WebRequest -Uri "http://127.0.0.1:$Port/json/version" -UseBasicParsing -TimeoutSec 2 | Out-Null
      $up = $true
      break
    } catch { Start-Sleep -Milliseconds 500 }
  }
  if (-not $up) {
    # Whatever the window did or failed to do, say it here rather than leaving
    # the next person to guess from a one-line "no".
    $proc.Refresh()
    if ($proc.HasExited) {
      Write-Host "    the app exited with code $($proc.ExitCode)" -ForegroundColor Yellow
    } else {
      Write-Host '    the app is still running but opened no port' -ForegroundColor Yellow
    }
    foreach ($pair in @(@('stdout', $outLog), @('stderr', $errLog))) {
      $text = if (Test-Path $pair[1]) { (Get-Content $pair[1] -Raw) } else { '' }
      Write-Host "    --- app $($pair[0]) ---"
      if ($text.Trim()) { Write-Host $text } else { Write-Host '    (empty)' }
    }
    Fail "the WebView never opened a debugging port on $Port"
  }
  Write-Host '    attached'

  # --------------------------------------------------------- 4. drive the window
  Step 4 'drive the window'
  & node (Join-Path $repo 'tools/drive-app.mjs') --port $Port --repo $scratch
  if ($LASTEXITCODE -ne 0) { Fail 'the driven flow failed (see above)' }

  # --------------------------------------------- 5. what it left on disk
  Step 5 'check what it wrote'
  $ws = Join-Path $scratch 'docs'
  foreach ($rel in 'blastradius.yaml', 'model/context.yaml', 'views/containers.yaml') {
    if (-not (Test-Path (Join-Path $ws $rel))) { Fail "the scaffold did not write $rel" }
  }
  $readmeAfter = [System.IO.File]::ReadAllBytes((Join-Path $scratch 'README.md'))
  if (-not [System.Linq.Enumerable]::SequenceEqual([byte[]]$readmeBefore, [byte[]]$readmeAfter)) {
    Fail 'the README the user already had was modified'
  }
  $facts = Get-ChildItem (Join-Path $ws 'model/derived') -Filter *.l4.json -ErrorAction SilentlyContinue
  if (-not $facts) { Fail 'introspection wrote no facts file' }
  Write-Host "    facts: $($facts.Name -join ', ')"

  # The workspace the app just built has to satisfy the CLI that validates it
  # in CI — the same binary, from the same install.
  $cli = Join-Path $staged 'blastradius.exe'
  $out = & $cli validate $ws 2>&1 | Out-String
  if ($LASTEXITCODE -ne 0) { Fail "the workspace it wrote does not validate:`n$out" }
  Write-Host "    $($out.Trim().Split("`n")[-1])"

  Write-Host "`nAPP SMOKE PASSED" -ForegroundColor Green
} finally {
  if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
  if ($acl) { Set-Acl $staged $acl }
  if ($scratch -and (Test-Path $scratch) -and -not $KeepScratch) {
    Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
  }
}
