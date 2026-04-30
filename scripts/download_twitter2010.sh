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

Phase 1 expects a text edge list at:
  data/twitter-2010/twitter-2010-t.txt

Direct compressed WebGraph reading is deferred to Phase 1.5.
EOF
