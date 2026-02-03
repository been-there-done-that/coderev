#!/usr/bin/env bash
set -euo pipefail

if [ ! -x target/release/coderev ]; then
  cargo build --release
fi
bench/run_coderev.sh
bench/run_rg.sh
bench/run_coderev.sh
bench/run_rag.sh
bench/run_qa.sh
