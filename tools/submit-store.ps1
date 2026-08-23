# Store submission via the Microsoft Store submission API
# (spec/msix-store-packaging.md, 0.3.0 theme 3).
#
# Exists because msstore-cli cannot submit a prebuilt package: its `publish`
# command insists on a recognizable MSBuild project (the UWP/WinUI detector
# literally runs MSBuild), and its `submission update` only PUTs JSON without
# uploading files. This script speaks the documented REST flow directly:
# create submission -> upload package zip to the SAS blob -> update the
# packages list -> commit -> poll.
#
#   .\tools\submit-store.ps1 -ProductId 9XXXXXXXXXXX -UploadZip pkg.msixupload
#                            [-Commit] [-ReplacePending]
#
# Without -Commit the submission is left as an uncommitted draft in Partner
# Center (the dry run). Credentials come from the environment:
# PARTNER_CENTER_TENANT_ID, PARTNER_CENTER_CLIENT_ID, PARTNER_CENTER_CLIENT_SECRET.
param(
  [Parameter(Mandatory)][string]$ProductId,
  [Parameter(Mandatory)][string]$UploadZip,
  [switch]$Commit,
  [switch]$ReplacePending
)
$ErrorActionPreference = 'Stop'

foreach ($name in 'PARTNER_CENTER_TENANT_ID', 'PARTNER_CENTER_CLIENT_ID', 'PARTNER_CENTER_CLIENT_SECRET') {
  if (-not [Environment]::GetEnvironmentVariable($name)) { throw "$name is not set" }
}
if (-not (Test-Path $UploadZip)) { throw "$UploadZip does not exist" }

# 1. Token (Entra client credentials for the Partner Center resource).
$token = (Invoke-RestMethod -Method Post `
    -Uri "https://login.microsoftonline.com/$env:PARTNER_CENTER_TENANT_ID/oauth2/token" `
    -Body @{
      grant_type    = 'client_credentials'
      client_id     = $env:PARTNER_CENTER_CLIENT_ID
      client_secret = $env:PARTNER_CENTER_CLIENT_SECRET
      resource      = 'https://manage.devcenter.microsoft.com'
    }).access_token
$headers = @{ Authorization = "Bearer $token" }
$base = "https://manage.devcenter.microsoft.com/v1.0/my/applications/$ProductId"

# 2. The app, and any pending submission in the way.
$app = Invoke-RestMethod -Uri $base -Headers $headers
Write-Host "app: $($app.primaryName) ($ProductId)"
if ($app.pendingApplicationSubmission) {
  $pendingId = $app.pendingApplicationSubmission.id
  if (-not $ReplacePending) {
    throw "pending submission $pendingId already exists — inspect it in Partner Center, or pass -ReplacePending to delete it"
  }
  Write-Host "deleting pending submission $pendingId"
  Invoke-RestMethod -Method Delete -Uri "$base/submissions/$pendingId" -Headers $headers | Out-Null
}

# 3. New submission — a clone of the last published one (listing metadata
#    carries over; only the packages change here).
$sub = Invoke-RestMethod -Method Post -Uri "$base/submissions" -Headers $headers -ContentType 'application/json'
Write-Host "created submission $($sub.id)"

# 4. Packages: retire every cloned package, add one entry per .msix in the zip.
foreach ($p in $sub.applicationPackages) { $p.fileStatus = 'PendingDelete' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path $UploadZip))
$newEntries = @($zip.Entries | Where-Object { $_.Name -like '*.msix' } | ForEach-Object {
    Write-Host "  package: $($_.Name)"
    [pscustomobject]@{ fileName = $_.Name; fileStatus = 'PendingUpload'; minimumDirectXVersion = 'None'; minimumSystemRam = 'None' }
  })
$zip.Dispose()
if (-not $newEntries) { throw "$UploadZip contains no .msix entries" }
$sub.applicationPackages = @($sub.applicationPackages) + $newEntries

# 5. Upload the zip to the submission's SAS blob. The documented '+' gotcha:
#    the SAS URL must have literal pluses percent-encoded.
$blobUrl = $sub.fileUploadUrl.Replace('+', '%2B')
Invoke-WebRequest -Method Put -Uri $blobUrl -InFile $UploadZip `
  -Headers @{ 'x-ms-blob-type' = 'BlockBlob' } | Out-Null
Write-Host "uploaded $((Get-Item $UploadZip).Length) bytes"

# 6. Push the updated submission JSON.
Invoke-RestMethod -Method Put -Uri "$base/submissions/$($sub.id)" -Headers $headers `
  -ContentType 'application/json' -Body ($sub | ConvertTo-Json -Depth 32) | Out-Null

if (-not $Commit) {
  Write-Host "dry run: submission $($sub.id) left as an uncommitted draft — inspect (then delete) it in Partner Center"
  exit 0
}

# 7. Commit and poll until the Store takes over (certification proceeds in
#    Partner Center as usual).
Invoke-RestMethod -Method Post -Uri "$base/submissions/$($sub.id)/commit" -Headers $headers -ContentType 'application/json' | Out-Null
do {
  Start-Sleep -Seconds 15
  $st = Invoke-RestMethod -Uri "$base/submissions/$($sub.id)/status" -Headers $headers
  Write-Host "status: $($st.status)"
} while ($st.status -eq 'CommitStarted')
if ($st.status -like '*Failed*') {
  $st.statusDetails | ConvertTo-Json -Depth 8 | Write-Host
  throw "submission $($sub.id) failed"
}
Write-Host "submission $($sub.id) accepted: $($st.status)"
