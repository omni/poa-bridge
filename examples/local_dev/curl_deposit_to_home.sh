#!/bin/bash

generate_post_data()
{
  cat <<EOF
{
  "jsonrpc": "2.0",
  "method": "eth_sendTransaction",
  "params": [{ "from": "0x00a329c0648769A73afAc7F9381E08FB43dBEA72",
   "to": "0xebd3944af37ccc6b67ff61239ac4fef229c8f69f",
   "value": "0x186a0"}],
  "id": 0
}
EOF
}

CURL='/usr/bin/curl'
HTTP="localhost:8550"

$CURL -i \
-H "Content-Type:application/json" \
-X POST --data "$(generate_post_data)" $HTTP
