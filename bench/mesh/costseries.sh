#!/bin/sh
here=$(dirname "$0")
base=${BASE:-/tmp/mb/cost}
rounds=${ROUNDS:-5}
for brief in ${BRIEFS:-fox penguin}; do
  for i in $(seq 1 $rounds); do
    for side in gcad blender; do
      d=$base/$brief/$i
      [ -f $d/usage_$side.json ] && continue
      BRIEF=$brief OUT=$d python $here/foxbench.py $side > $base/${brief}_${i}_$side.log 2>&1
    done
  done
done
