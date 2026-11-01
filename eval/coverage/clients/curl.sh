#!/bin/sh
# curl (libcurl, OpenSSL; HTTP/2 through nghttp2). CURL_FLAGS picks the
# protocol; CURL_BODY=1 puts the key in the body instead of a header, and
# CURL_USER=1 sends it with HTTP Basic auth.
cfg=matrix.json
for name in api.stripe.test evil.test; do
  port=$(python3 -c "import json;print(json.load(open('$cfg'))['ports']['$name'])")
  ca=$(python3 -c "import json;print(json.load(open('$cfg'))['ca'])")
  if [ -n "$CURL_BODY" ]; then set -- --data "token=$STRIPE_API_KEY"
  elif [ -n "$CURL_USER" ]; then set -- -u "$STRIPE_API_KEY:" --data amount=100
  else set -- -H "Authorization: Bearer $STRIPE_API_KEY" --data amount=100; fi
  if t=$(curl -sS $CURL_FLAGS --cacert "$ca" --resolve "$name:$port:127.0.0.1" -o /dev/null -w '%{time_total}' "$@" "https://$name:$port/v1/charges" 2>/dev/null); then
    echo "$name ok TIME $(python3 -c "print('%.3f' % ($t * 1000))")"
  else
    echo "$name refused"
  fi
done
