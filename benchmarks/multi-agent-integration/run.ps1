[CmdletBinding()]
param([switch]$KeepArtifacts)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$runRoot = Join-Path ([IO.Path]::GetTempPath()) ('agent-hud-issue-100-' + [guid]::NewGuid())
$repoRoot = Join-Path $runRoot 'repo'
$profile = Join-Path $runRoot 'profile'
$target = Join-Path $runRoot 'target'
$result = Join-Path $runRoot 'result.json'
$originalUserProfile = $env:USERPROFILE
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $originalUserProfile '.rustup' }
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $originalUserProfile '.cargo' }

try {
    New-Item -ItemType Directory -Force -Path $runRoot, $profile, $target | Out-Null
    git clone --local --no-hardlinks $repo $repoRoot | Out-Null
    git -C $repoRoot switch -c benchmark/issue-100 | Out-Null
    git -C $repoRoot config user.email benchmark@example.invalid
    git -C $repoRoot config user.name agent-hud-benchmark
    Set-Content (Join-Path $repoRoot 'benchmark-seed.txt') 'issue-100'
    git -C $repoRoot add benchmark-seed.txt
    git -C $repoRoot commit -m 'benchmark seed' | Out-Null
    git -C $repoRoot worktree add (Join-Path $runRoot 'worker-alpha') HEAD | Out-Null
    git -C $repoRoot worktree add (Join-Path $runRoot 'worker-beta') HEAD | Out-Null

    $env:USERPROFILE = $profile
    $env:HOMEDRIVE = $null
    $env:HOMEPATH = $null
    $env:CARGO_TARGET_DIR = $target
    $env:RUSTUP_HOME = $rustupHome
    $env:CARGO_HOME = $cargoHome
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Push-Location $repoRoot
    cargo test benchmark::multi_agent_fixture_covers_grouping_readiness_timeline_and_risk --manifest-path (Join-Path $repoRoot 'Cargo.toml') -- --exact --nocapture
    $cargoExit = $LASTEXITCODE
    Pop-Location
    $watch.Stop()
    if ($cargoExit -ne 0) { throw "focused benchmark test failed with exit code $cargoExit" }

    [pscustomobject]@{
        scenario = 'multi-agent-integration'
        issue = 100
        worktrees = 2
        focused_test = 'passed'
        runner_elapsed_ms = $watch.Elapsed.TotalMilliseconds
        artifacts_root = $runRoot
    } | ConvertTo-Json | Set-Content $result
    Get-Content $result
    if (-not $KeepArtifacts) { Remove-Item -LiteralPath $runRoot -Recurse -Force }
}
catch {
    if (Test-Path $runRoot) { Write-Error "Benchmark failed; isolated artifacts retained at $runRoot" }
    throw
}
