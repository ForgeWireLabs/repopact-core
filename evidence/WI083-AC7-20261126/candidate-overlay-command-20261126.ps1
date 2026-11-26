$ErrorActionPreference = 'Stop'
$fixture = 'C:\Projects\repopact-core-wi083-ac7-candidate-fixture-overlay-559ed9a-20261126'
$harness = 'C:\Projects\repopact-core-wi083-ac7-candidate-harness-20261126'
$fixtureHead = git -C $fixture rev-parse HEAD
if ($fixtureHead -ne '559ed9a86a1c399826f9505e5412d11c54d099ab') { throw "Unexpected fixture HEAD: $fixtureHead" }
$fixtureChanges = git -C $fixture diff --name-only
$expected = @('rust/crates/repopact-repository/src/lib.rs', 'rust/crates/repopact-mutation/src/lib.rs')
if (($fixtureChanges | Sort-Object -Unique).Count -ne 2 -or $fixtureChanges -notcontains $expected[0] -or $fixtureChanges -notcontains $expected[1]) { throw "Unexpected fixture overlay: $fixtureChanges" }
$expectedRepoHash = '21C614B3AEEEF6F2462FA125C260A2CBA0AA1FE3D0D433A422BF0ADBDDFDB3F9'
$expectedMutationHash = '444059C5D78DCFCEBCF66A4E770BB52C6F559BD6C610D25C66219AF8DE3226B2'
if ((Get-FileHash -Algorithm SHA256 (Join-Path $fixture 'rust\crates\repopact-repository\src\lib.rs')).Hash -ne $expectedRepoHash -or (Get-FileHash -Algorithm SHA256 (Join-Path $fixture 'rust\crates\repopact-mutation\src\lib.rs')).Hash -ne $expectedMutationHash) { throw 'Shared fixture source hashes changed' }
$harnessChanges = git -C $harness diff --name-only
if (($harnessChanges | Sort-Object -Unique).Count -ne 2 -or $harnessChanges -notcontains $expected[0] -or $harnessChanges -notcontains $expected[1]) { throw "Unexpected harness diff: $harnessChanges" }
$env:CARGO_TARGET_DIR = 'C:\Projects\repopact-core-wi083-ac7-candidate-overlay-rust-target-20261126'
$env:WI083_AC7_FIXTURE_ROOT = $fixture
$log = 'C:\Projects\repopact-core-wi083-ac7-candidate-overlay-clean-raw-20261126.txt'
Write-Output "CANDIDATE_OVERLAY_BEGIN,$(Get-Date -Format o),fixture=$fixture,head=$fixtureHead,fixture_overlay=repository+mutation-source,harness=$harness"
cargo test --locked --manifest-path rust\Cargo.toml -p repopact-mutation wi083_ac7_original_baseline_measurement -- --test-threads=1 --nocapture 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Output "CANDIDATE_OVERLAY_END,$(Get-Date -Format o)"