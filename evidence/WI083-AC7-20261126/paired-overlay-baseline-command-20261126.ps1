$ErrorActionPreference = 'Stop'
$fixture = 'C:\Projects\repopact-core-wi083-ac7-candidate-fixture-overlay-559ed9a-20261126'
$harness = 'C:\Projects\repopact-core-wi083-ac7-candidate-overlay-baseline-harness-20261126'
$fixtureHead = git -C $fixture rev-parse HEAD
if ($fixtureHead -ne '559ed9a86a1c399826f9505e5412d11c54d099ab') { throw "Unexpected fixture HEAD: $fixtureHead" }
$expected = @('rust/crates/repopact-repository/src/lib.rs', 'rust/crates/repopact-mutation/src/lib.rs')
$fixtureChanges = git -C $fixture diff --name-only
if (($fixtureChanges | Sort-Object -Unique).Count -ne 2 -or $fixtureChanges -notcontains $expected[0] -or $fixtureChanges -notcontains $expected[1]) { throw "Unexpected fixture overlay: $fixtureChanges" }
$harnessChanges = git -C $harness diff --name-only
if (@($harnessChanges).Count -ne 1 -or $harnessChanges -notcontains 'rust/crates/repopact-mutation/src/lib.rs') { throw "Unexpected baseline harness diff: $harnessChanges" }
$env:CARGO_TARGET_DIR = 'C:\Projects\repopact-core-wi083-ac7-paired-baseline-rust-target-20261126'
$env:WI083_AC7_FIXTURE_ROOT = $fixture
$log = 'C:\Projects\repopact-core-wi083-ac7-paired-overlay-baseline-raw-20261126.txt'
Write-Output "PAIRED_OVERLAY_BASELINE_BEGIN,$(Get-Date -Format o),fixture=$fixture,head=$fixtureHead,fixture_overlay=repository+mutation-source,harness=$harness"
cargo test --locked --manifest-path rust\Cargo.toml -p repopact-mutation wi083_ac7_original_baseline_measurement -- --test-threads=1 --nocapture 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Output "PAIRED_OVERLAY_BASELINE_END,$(Get-Date -Format o)"