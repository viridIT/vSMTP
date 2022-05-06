#!/bin/bash
# counts instructions for a standard workload
set -e

OUTFILE="tmp/cachegrind.stress.`git describe --always --dirty`-`date +%s`"

cargo build \
  --bin=vsmtp \
  --release

# --tool=callgrind --dump-instr=yes --collect-jumps=yes --simulate-cache=yes \
# --callgrind-out-file="$OUTFILE" \

valgrind \
  --tool=cachegrind \
  --cachegrind-out-file="$OUTFILE" \
  ./target/release/vsmtp \
  -t 10s --no-daemon -c ./benchmarks/stress/vsmtp.stress.toml

LAST=`ls -t tmp/cachegrind.stress.* | sed -n 2p`

echo "comparing $LAST with new $OUTFILE"

echo "--------------------------------------------------------------------------------"
echo "change since last run:"
echo "         Ir   I1mr  ILmr          Dr    D1mr    DLmr          Dw    D1mw    DLmw"
echo "--------------------------------------------------------------------------------"
cg_diff $LAST $OUTFILE | tail -1
