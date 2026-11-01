#!/bin/sh
# curl with HTTP Basic auth (-u key:), as Stripe's own examples do: the key is base64-encoded.
CURL_USER=1 exec sh curl.sh
