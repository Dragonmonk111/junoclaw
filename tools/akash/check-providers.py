import subprocess, json

# Check active providers
result = subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "query", "provider", "list",
     "--node", "https://akash-rpc.polkachu.com:443", "--output", "json"],
    capture_output=True, text=True, timeout=30
)
data = json.loads(result.stdout)
providers = data.get("providers", [])
print(f"Total providers: {len(providers)}")

# Check active GPU leases to see which providers actually serve GPUs
lease_result = subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "query", "market", "lease", "list",
     "--node", "https://akash-rpc.polkachu.com:443", "--output", "json", "--state", "active", "--limit", "200"],
    capture_output=True, text=True, timeout=30
)
lease_data = json.loads(lease_result.stdout)
leases = lease_data.get("leases", [])
print(f"Active leases: {len(leases)}")

gpu_providers = set()
for l in leases:
    lease = l.get("lease", {})
    lid = lease.get("id", lease.get("lease_id", {}))
    provider = lid.get("provider", "?")
    # Check if the deployment has GPU requirements
    dseq = lid.get("dseq", "?")
    # We'd need to query each deployment to check GPU, but let's just list active lease providers
    gpu_providers.add(provider)

print(f"Providers with active leases: {len(gpu_providers)}")
for p in sorted(gpu_providers)[:20]:
    print(f"  {p}")
