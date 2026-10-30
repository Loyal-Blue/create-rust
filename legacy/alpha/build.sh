#!/usr/bin/env bash
set -euo pipefail
task_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd -- "$task_root"
command -v cargo-horizon-mod >/dev/null || { echo 'Install cargo-horizon-mod as described in README.md' >&2; exit 1; }
python3 tools/validate_content.py
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
(cd content && cargo horizon-mod build)
(cd server && cargo horizon-mod build && cargo horizon-mod test --hmod ../target/horizon-mod/create_logic.hmod)
mkdir -p dist
cp content/target/horizon-mod/create.hmod dist/create.hmod
cp target/horizon-mod/create_logic.hmod dist/create_logic.hmod
cargo horizon-mod check dist/create.hmod --json > dist/content-check.json
cargo horizon-mod check dist/create_logic.hmod --json > dist/logic-check.json
echo 'Built and checked dist/create.hmod and dist/create_logic.hmod'
