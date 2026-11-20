if (-not $IsLinux) {
  throw "This script is intended to run on Linux systems only."
}

cargo +nightly kani list -Z stubbing
cargo +nightly kani --fail-fast -Z stubbing
cargo +nightly kani --fail-fast -Z stubbing --harness xyz
cargo +nightly kani -Z stubbing -Z concrete-playback --concrete-playback=inplace --harness xyz
