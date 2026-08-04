import subprocess, json, sys, time, os, tempfile, shutil

# Get mnemonic from WalletStore
result = subprocess.run(
    ["node", "-e", """
const { getDefaultWalletStore } = require('./mcp/dist/wallet/store.js');
getDefaultWalletStore().exportMnemonicForExternalSigner('akash-jlens').then(m => process.stdout.write(m));
"""],
    capture_output=True, text=True,
    cwd="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw"
)
mnemonic = result.stdout.strip()

# Create temp keyring
tmpdir = tempfile.mkdtemp(prefix="akash-keyring-")
wsl_dir = tmpdir.replace("C:", "/mnt/c").replace("\\", "/")
mnemonic_file = os.path.join(tmpdir, "mnemonic.txt")
with open(mnemonic_file, "w") as f:
    f.write(mnemonic + "\n")

subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "keys", "add",
     "junoclaw-autodeploy", "--recover", "--keyring-backend", "test",
     "--home", wsl_dir],
    input=mnemonic + "\n", encoding="utf-8", timeout=30
)

# Create cert
subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "tx", "cert", "generate", "client",
     "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2",
     "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wsl_dir,
     "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"],
    capture_output=True, text=True, timeout=30
)
subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "tx", "cert", "publish", "client",
     "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2",
     "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wsl_dir,
     "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"],
    capture_output=True, text=True, timeout=60
)
time.sleep(10)

# Create deployment
deploy_result = subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "tx", "deployment", "create",
     "/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/tools/akash/sdl-mixtral-8x7b.yml",
     "--deposit", "5000000uact",
     "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2",
     "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wsl_dir,
     "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"],
    capture_output=True, text=True, timeout=120
)
deploy_data = json.loads(deploy_result.stdout)
txhash = deploy_data.get("txhash", "?")
print(f"Deployment tx: {txhash}")

# Query deployments to get dseq
dep_result = subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "query", "deployment", "list",
     "--owner", "akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt",
     "--node", "https://akash-rpc.polkachu.com:443", "--output", "json", "--state", "active"],
    capture_output=True, text=True, timeout=30
)
dep_data = json.loads(dep_result.stdout)
deps = dep_data.get("deployments", [])
if deps:
    dep = deps[0].get("deployment", {})
    dseq = dep.get("id", {}).get("dseq", "?")
    print(f"dseq: {dseq}")
    
    # Wait 30s for bids
    print("Waiting 30s for bids...")
    time.sleep(30)
    
    # Query bids
    bid_result = subprocess.run(
        ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "query", "market", "bid", "list",
         "--owner", "akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt",
         "--dseq", dseq,
         "--node", "https://akash-rpc.polkachu.com:443", "--output", "json"],
        capture_output=True, text=True, timeout=30
    )
    bid_data = json.loads(bid_result.stdout)
    bids = bid_data.get("bids", [])
    print(f"Bids: {len(bids)}")
    if bids:
        print(f"First bid structure:")
        print(json.dumps(bids[0], indent=2)[:3000])
        
        # Close deployment
        subprocess.run(
            ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "tx", "deployment", "close",
             "--dseq", dseq,
             "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2",
             "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wsl_dir,
             "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"],
            capture_output=True, text=True, timeout=60
        )
        print("Deployment closed.")
else:
    print("No active deployments found")

# Cleanup
os.remove(mnemonic_file)
shutil.rmtree(tmpdir, ignore_errors=True)
