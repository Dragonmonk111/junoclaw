# Deploying WAVS Operator on Akash

## Prerequisites

- **Wallet**: `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`
- **Balance**: ~63.77 AKT (confirmed)
- **Est. cost**: ~$5-8/month for all 3 services
- **SDL file**: `wavs/akash.sdl.yml`

## Deployment Steps (Akash Console)

### 1. Open Akash Console

Go to [console.akash.network](https://console.akash.network) and connect your wallet using Keplr.

### 2. Import SDL

- Click **"Deploy"** → **"Build your template"** → **"Import SDL"**
- Paste the contents of `wavs/akash.sdl.yml`
- Review the 3 services: `wavs-operator`, `wavs-aggregator`, `ipfs`

### 3. Set Deposit

- Akash requires a 5 AKT deposit to create a deployment
- The deposit is refundable when you close the deployment
- Actual cost is billed per block from the deposit

### 4. Select Provider

- Choose a provider with the best price-to-specs ratio
- Prefer providers with `host: amd` attribute (matching our SDL)
- Typical cost: ~0.5-1 AKT/day for all 3 services

### 5. Accept Lease & Deploy

- Review the lease terms
- Accept the lease — containers will start spinning up
- Wait for all 3 services to show "Running" status

### 6. Get Endpoints

After deployment, Akash Console will show:
- **Aggregator** public endpoint (the only globally-exposed port): `https://<provider>.akash.network:8080`
- Internal services communicate via DNS names (`wavs-operator:8041`, `ipfs:5001`)

### 7. Verify

```bash
# Check aggregator is responding
curl https://<aggregator-endpoint>/health

# Check operator is registered
curl https://<aggregator-endpoint>/operators
```

### 8. Update Bridge Config

Once deployed, update `wavs/bridge/.env` with:

```
WAVS_AGGREGATOR_URL=https://<aggregator-endpoint>
```

The bridge daemon will then submit attestations via the Akash-hosted aggregator instead of localhost.

## Environment Variables

The SDL includes these pre-configured values:

| Variable | Value | Notes |
|----------|-------|-------|
| `WAVS_SERVICE_ID` | `572b188a...` | From WAVS service registration |
| `WAVS_CHAIN_RPC` | `https://rpc.uni.junonetwork.io:443` | Juno testnet |
| `WAVS_CONTRACT` | `juno1k8dxll...stj85k6` | agent-company contract |
| `WAVS_CHAIN_ID` | `uni-7` | Juno testnet chain ID |
| `WAVS_COMPONENT_REGISTRY` | `wa.dev` | WASI component registry |

## Monitoring

After deployment:
1. Check logs in Akash Console → Deployment → Logs tab
2. Monitor AKT spend in the deployment details
3. The aggregator endpoint should be reachable globally
4. Verify attestations are being submitted to uni-7

## Teardown

To stop and reclaim your deposit:
1. Go to Akash Console → Deployments
2. Click your deployment → **Close Deployment**
3. 5 AKT deposit will be returned minus usage
