#!/usr/bin/env bash
set -euo pipefail
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ $# != 1 ]]; then
  echo "Usage: $0 /absolute/path/to/horizon" >&2
  exit 2
fi
engine_root=$(cd -- "$1" && pwd)
python3 "$project_root/engine/apply.py" "$engine_root"
python3 "$project_root/tools/prepare_native_content.py"
cargo test --manifest-path "$project_root/Cargo.toml" -p create-runtime -p create-kinetics -p create-simulation --locked
cargo test --manifest-path "$engine_root/Cargo.toml" -p horizon-client --lib --no-default-features --features create create_machine_slots_and_native_data_roundtrip --locked
cargo build --manifest-path "$engine_root/Cargo.toml" --profile fast -p horizon-client --bin horizon-client --no-default-features --features create --locked
printf 'Engine: %s/target/fast/horizon-client\nContent: %s/dist/create-native-content.hmod\nRead NATIVE_DEVELOPMENT.md before launching.\n' "$engine_root" "$project_root"
