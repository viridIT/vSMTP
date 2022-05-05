#!/bin/bash
# counts instructions for a standard workload
set -e

OUTFILE="tmp/cachegrind.stress.`git describe --always --dirty`-`date +%s`"

cargo build \
  --bin=vsmtp-stress \
  --release

# --tool=callgrind --dump-instr=yes --collect-jumps=yes --simulate-cache=yes \
# --callgrind-out-file="$OUTFILE" \

valgrind \
  --tool=cachegrind \
  --cachegrind-out-file="$OUTFILE" \
  ./target/release/vsmtp-stress \
  --total-ops=50000 --set-prop=1000000000000 --threads=1

LAST=`ls -t tmp/cachegrind.stress.* | sed -n 2p`

echo "comparing $LAST with new $OUTFILE"

echo "--------------------------------------------------------------------------------"
echo "change since last run:"
echo "         Ir   I1mr  ILmr          Dr    D1mr    DLmr          Dw    D1mw    DLmw"
echo "--------------------------------------------------------------------------------"
cg_diff $LAST $OUTFILE | tail -1
