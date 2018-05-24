#!/bin/bash

generate_post_data()
{
  cat <<EOF
{
  "jsonrpc": "2.0",
  "method": "parity_newAccountFromPhrase",
  "params": ["node0", ""],
  "id": 0
}
EOF
}

CURL='/usr/bin/curl'
HTTP="localhost:8550"

$CURL -i \
-H "Content-Type:application/json" \
-X POST --data "$(generate_post_data)" $HTTP
