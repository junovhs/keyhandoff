#!/bin/sh
# curl over plain HTTP to a stranger: the placeholder in clear text.
port=$(python3 -c "import json;print(json.load(open('matrix.json'))['ports']['plain'])")
if curl -sS -o /dev/null -H "Authorization: Bearer $STRIPE_API_KEY" "http://127.0.0.1:$port/collect" 2>/dev/null; then echo "plain ok"; else echo "plain refused"; fi
