#!/bin/sh
set -eu
cd "$(dirname "$0")"

rustqec circuit gen \
  --code surface_code --task rotated_memory_z_midswap \
  --distance 3 --rounds 2 --noise 0.002 \
  --operation-loss-probability 0.002 \
  --measurement-loss-probability 0.003 \
  --out circuit.stim

mkdir -p data
rustqec dataset export \
  --circuit circuit.stim --shots 64 --seed 7 \
  --mode measurements_blinded --logical-x-qubits 1,8,15 \
  --public-out data/public --private-out data/private

rustqec decode --decoder envelope-matching \
  --dataset data/public \
  --out predictions.b8 --stats-out decode-stats.json

python3 check.py
