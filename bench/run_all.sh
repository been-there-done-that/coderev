#!/usr/bin/env bash
set -euo pipefail

bench/run_coderev.sh
bench/run_rg.sh
bench/run_coderev.sh
bench/run_rag.sh
bench/run_qa.sh
