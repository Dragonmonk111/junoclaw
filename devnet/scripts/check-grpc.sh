#!/bin/bash
echo "=== gRPC section in app.toml ==="
grep -A5 '^\[grpc\]' /root/.juno/config/app.toml

echo ""
echo "=== All enable lines ==="
grep -n 'enable' /root/.juno/config/app.toml | head -20

echo ""
echo "=== Test gRPC port ==="
curl -sf http://localhost:9090 2>&1 | head -5 || echo "curl 9090: $?"

echo ""
echo "=== Listening ports ==="
ss -tlnp 2>/dev/null | grep -E '9090|1317|26657' || netstat -tlnp 2>/dev/null | grep -E '9090|1317|26657' || echo "no ss/netstat"
