#!/bin/bash
# Sets up warg-server as a systemd service on Azure VM
# After this: registry auto-starts on boot + auto-publishes component
set -e

source ~/.cargo/env
CARGO_BIN=$(which warg-server)
WARG_CLI=$(which warg)
OPKEY=$(cat ~/.config/warg/keyring/'service=warg-signing-key&user=default')

echo "=== Creating systemd service for warg-server ==="

sudo tee /etc/systemd/system/warg-registry.service > /dev/null << EOF
[Unit]
Description=Warg Component Registry for JunoClaw
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=azureuser
Environment=HOME=/home/azureuser
ExecStart=$CARGO_BIN --listen 0.0.0.0:8090 --content-dir /home/azureuser/warg-content --operator-key $OPKEY --namespace junoclaw
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

echo "=== Creating auto-publish service (runs after registry starts) ==="

sudo tee /etc/systemd/system/warg-publish.service > /dev/null << EOF
[Unit]
Description=Auto-publish JunoClaw WASI component to warg registry
After=warg-registry.service
Requires=warg-registry.service

[Service]
Type=oneshot
User=azureuser
Environment=HOME=/home/azureuser
Environment=PATH=/home/azureuser/.cargo/bin:/usr/local/bin:/usr/bin:/bin
ExecStartPre=/bin/sleep 5
ExecStart=/bin/bash /home/azureuser/auto-publish.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF

echo "=== Creating auto-publish script ==="

cat > ~/auto-publish.sh << 'PUBLISH'
#!/bin/bash
source ~/.cargo/env

# Ensure client points to local registry
warg config --registry http://localhost:8090 --keyring-backend flat-file --overwrite

# Wait for registry to be ready
for i in $(seq 1 30); do
  if curl -s http://localhost:8090 > /dev/null 2>&1; then
    echo "Registry is ready"
    break
  fi
  echo "Waiting for registry... ($i/30)"
  sleep 2
done

# Abort any stale publish
warg publish abort 2>/dev/null || true

# Publish component
echo "Publishing junoclaw:verifier v0.1.0..."
warg publish start junoclaw:verifier
warg publish init junoclaw:verifier
warg publish release --name junoclaw:verifier --version 0.1.0 ~/junoclaw_wavs_component.wasm
warg publish submit

echo "=== Verifying ==="
warg info junoclaw:verifier

echo "=== Auto-publish complete at $(date) ==="
PUBLISH

chmod +x ~/auto-publish.sh

echo "=== Stopping any manual warg-server processes ==="
pkill -f warg-server 2>/dev/null || true
sleep 2

echo "=== Enabling and starting services ==="
sudo systemctl daemon-reload
sudo systemctl enable warg-registry.service
sudo systemctl enable warg-publish.service
sudo systemctl start warg-registry.service
sleep 3
sudo systemctl start warg-publish.service

echo "=== Service status ==="
sudo systemctl status warg-registry.service --no-pager | head -15
echo "---"
sudo systemctl status warg-publish.service --no-pager | head -15

echo "=== Verifying from client ==="
warg config --registry http://localhost:8090 --keyring-backend flat-file --overwrite
warg info junoclaw:verifier

echo ""
echo "========================================="
echo " DONE! Registry will auto-start on boot"
echo " Component will auto-publish on boot"
echo "========================================="
