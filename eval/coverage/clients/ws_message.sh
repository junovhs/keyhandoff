#!/bin/sh
# python3, or python where that is its name (Windows).
PY=python3; "$PY" -c "" 2>/dev/null || PY=python
WS_PART=message exec "$PY" py_websocket.py
