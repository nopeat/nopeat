param(
  [Parameter(Mandatory = $true)][string]$Binary,
  [Parameter(Mandatory = $true)][string]$Target,
  [string]$Extra = '',
  [int]$Runs = 1
)

function Quote-Native([string]$a) {
  if ($a -notmatch '[\s"]') { return $a }
  return '"' + ($a -replace '(\\*)"', '$1$1\"' -replace '(\\+)$', '$1$1') + '"'
}

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Binary
$psi.Arguments = ((Quote-Native $Target) + ' ' + $Extra).Trim()
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false

$walls = @()
$peaks = @()
$privates = @()
$lastOut = ''
$lastErr = ''

for ($i = 0; $i -lt $Runs; $i++) {
  $proc = [System.Diagnostics.Process]::Start($psi)
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $stdout = $proc.StandardOutput.ReadToEndAsync()
  $stderr = $proc.StandardError.ReadToEndAsync()

  $peakWs = 0
  $peakPrivate = 0
  while (-not $proc.HasExited) {
    try {
      $proc.Refresh()
      $ws = [math]::Round($proc.WorkingSet64 / 1MB, 1)
      if ($ws -gt $peakWs) { $peakWs = $ws }
      $priv = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
      if ($priv -gt $peakPrivate) { $peakPrivate = $priv }
    } catch { }
    Start-Sleep -Milliseconds 25
  }
  $sw.Stop()
  $lastOut = $stdout.Result
  $lastErr = $stderr.Result
  $walls += $sw.ElapsedMilliseconds
  $peaks += $peakWs
  $privates += $peakPrivate
  $proc.Dispose()
}

$mid = [int][math]::Floor($Runs / 2)
$sortedW = @($walls | Sort-Object)
$sortedP = @($peaks | Sort-Object)
$sortedPriv = @($privates | Sort-Object)

$out = [ordered]@{
  binary             = $Binary
  target             = $Target
  args               = $psi.Arguments
  runs               = $Runs
  wall_ms            = @($walls)
  wall_ms_median     = $sortedW[$mid]
  peak_rss_mb        = @($peaks)
  peak_rss_median_mb = $sortedP[$mid]
  peak_private_mb        = @($privates)
  peak_private_median_mb = $sortedPriv[$mid]
  rss_source         = 'sampled max every 25 ms; working set includes file-backed pages, private bytes do not. Not a kernel high-water mark.'
  stdout_tail        = (($lastOut -split "`r?`n" | Where-Object { $_.Trim() } | Select-Object -First 3) -join ' | ')
  stderr_tail        = (($lastErr -split "`r?`n" | Where-Object { $_.Trim() } | Select-Object -First 3) -join ' | ')
}
$out | ConvertTo-Json -Depth 4 -Compress
