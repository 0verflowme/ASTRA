#!/usr/bin/env bash
set -euo pipefail

mkdir -p data/twitter-2010
cd data/twitter-2010

wget -c https://data.law.di.unimi.it/webdata/twitter-2010/twitter-2010-t.graph
wget -c https://data.law.di.unimi.it/webdata/twitter-2010/twitter-2010-t.properties
wget -c https://data.law.di.unimi.it/webdata/twitter-2010/twitter-2010.outdegree
wget -c https://data.law.di.unimi.it/webdata/twitter-2010/twitter-2010.indegree

cat <<'EOF'
Downloaded compressed WebGraph files.

Run the compressed graph directly with:
  cargo run --release -- run-bvgraph --basename data/twitter-2010/twitter-2010-t --limit-edges 100000000 --progress-every 10000000

Sequential BVGraph streaming requires .graph and .properties only. It does not require a .ef random-access index.
EOF
