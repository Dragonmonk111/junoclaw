const { readFileSync, existsSync, writeFileSync } = require('fs');
const { join } = require('path');
const crypto = require('crypto');
const { DirectSecp256k1HdWallet, coins, Registry } = require('@cosmjs/proto-signing');
const { GasPrice, calculateFee, defaultRegistryTypes, SigningStargateClient } = require('@cosmjs/stargate');
const { SigningCosmWasmClient } = require('@cosmjs/cosmwasm-stargate');
let protobuf;
try { protobuf = require('protobufjs'); } catch (e) {
  console.error('protobufjs not found. Run: cd deploy && npm install protobufjs');
  process.exit(1);
}

const PARLIAMENT_STATE = join(__dirname, '..', 'wavs', 'bridge', 'parliament-state.json');
const DEPLOYED_FILE = join(__dirname, 'deployed-testnet.json');
const DEVNET_DIR = join(__dirname, '..', 'devnet');

function loadMnemonic() {
  if (process.env.JUNO_MNEMONIC) return process.env.JUNO_MNEMONIC;
  if (process.env.MNEMONIC) return process.env.MNEMONIC;
  const role = process.env.PARLIAMENT_ROLE || 'The Builder';
  if (!existsSync(PARLIAMENT_STATE)) {
    console.error(`Error: ${PARLIAMENT_STATE} not found`);
    process.exit(1);
  }
  const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'));
  const mp = (state.mps || []).find((m) => m.name === role);
  if (!mp) {
    console.error(`No MP named "${role}" in parliament-state.json`);
    process.exit(1);
  }
  console.log(`Wallet: ${role} (${mp.address})`);
  return mp.mnemonic;
}

const MNEMONIC = loadMnemonic();

const CHAIN_ID = 'uni-7';
const RPC_URL = process.env.RPC_URL || 'https://juno.rpc.t.stavr.tech';
const REST_URL = 'https://juno-testnet-api.cogwheel.zone';
const GAS_PRICE = '0.075ujunox';

// Proto definitions for FeePay messages
const protoRoot = protobuf.Root.fromJSON({
  nested: {
    cosmos: {
      nested: {
        base: {
          nested: {
            v1beta1: {
              nested: {
                Coin: {
                  fields: {
                    denom: { type: 'string', id: 1 },
                    amount: { type: 'string', id: 2 },
                  },
                },
              },
            },
          },
        },
      },
    },
    juno: {
      nested: {
        feepay: {
          nested: {
            v1: {
              nested: {
                FeePayContract: {
                  fields: {
                    contractAddress: { type: 'string', id: 1 },
                    balance: { type: 'uint64', id: 2 },
                    walletLimit: { type: 'uint64', id: 3 },
                  },
                },
                MsgRegisterFeePayContract: {
                  fields: {
                    senderAddress: { type: 'string', id: 1 },
                    feePayContract: { type: 'FeePayContract', id: 2 },
                  },
                },
                MsgFundFeePayContract: {
                  fields: {
                    senderAddress: { type: 'string', id: 1 },
                    contractAddress: { type: 'string', id: 2 },
                    amount: { rule: 'repeated', type: 'cosmos.base.v1beta1.Coin', id: 3 },
                  },
                },
              },
            },
          },
        },
      },
    },
  },
});

const MsgRegister = protoRoot.lookupType('juno.feepay.v1.MsgRegisterFeePayContract');
const MsgFund = protoRoot.lookupType('juno.feepay.v1.MsgFundFeePayContract');

// Registry with default types + FeePay types (for stargate client)
const feePayRegistry = new Registry([
  ...defaultRegistryTypes,
  ['/juno.feepay.v1.MsgRegisterFeePayContract', {
    encode: (msg, writer) => MsgRegister.encode(msg, writer),
    decode: (reader, len) => MsgRegister.decode(reader, len),
    fromPartial: (o) => o, fromJSON: (o) => o, toJSON: (m) => m,
  }],
  ['/juno.feepay.v1.MsgFundFeePayContract', {
    encode: (msg, writer) => MsgFund.encode(msg, writer),
    decode: (reader, len) => MsgFund.decode(reader, len),
    fromPartial: (o) => o, fromJSON: (o) => o, toJSON: (m) => m,
  }],
]);

async function main() {
  console.log('=== FeePay Test on uni-7 (v2) ===\n');
  console.log('Chain:', CHAIN_ID);
  console.log('RPC:', RPC_URL);
  console.log();

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' });
  const [{ address: sender }] = await wallet.getAccounts();
  console.log('Sender:', sender);

  // Client 1: SigningStargateClient with FeePay registry (for FeePay messages)
  const stargateClient = await SigningStargateClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
    registry: feePayRegistry,
  });

  // Client 2: SigningCosmWasmClient without custom registry (for CosmWasm operations)
  const cosmwasmClient = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  });

  const balance = await cosmwasmClient.getBalance(sender, 'ujunox');
  console.log('Balance:', balance.amount, balance.denom);

  if (BigInt(balance.amount) < 5000000n) {
    console.error('ERROR: Need at least 5 JUNOX for FeePay testing.');
    process.exit(1);
  }
  console.log();

  // --- Step 1: Check FeePay params ---
  console.log('--- Step 1: Check FeePay params ---');
  try {
    const resp = await fetch(`${REST_URL}/juno/feepay/v1/params`);
    const params = await resp.json();
    console.log('FeePay enabled:', params.params?.enable_feepay);
    if (!params.params?.enable_feepay) {
      console.error('ERROR: FeePay is not enabled on uni-7');
      process.exit(1);
    }
  } catch (e) {
    console.error('Could not query FeePay params:', e.message);
  }
  console.log();

  // --- Step 2: Instantiate a NEW moultbook with admin=sender ---
  console.log('--- Step 2: Instantiate fresh moultbook with admin=sender ---');
  const deployed = JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'));
  const moultbookCodeId = deployed['moultbook']?.codeId;
  if (!moultbookCodeId) {
    console.error('ERROR: moultbook codeId not found in deployed-testnet.json');
    process.exit(1);
  }
  console.log('Using moultbook codeId:', moultbookCodeId);

  const zkVerifierAddr = deployed['zk-verifier-pure']?.address || null;
  const jclawCredentialAddr = deployed['jclaw-credential']?.address || null;

  const instantiateMsg = {
    admin: sender,
    whoami_contract: null,
    max_size_bytes: 1048576,
    max_refs: 8,
    max_content_type_len: 64,
    max_group_size: 50,
    zk_verifier: zkVerifierAddr,
    agent_registry: jclawCredentialAddr,
    membership_vk_hash: null,
  };

  const instFee = calculateFee(500000, GasPrice.fromString(GAS_PRICE));
  let newMoultbookAddr = null;
  try {
    const res = await cosmwasmClient.instantiate(
      sender, moultbookCodeId, instantiateMsg, 'feepay-test-moultbook', instFee, { admin: sender }
    );
    newMoultbookAddr = res.contractAddress;
    console.log('New moultbook:', newMoultbookAddr);
    console.log('Instantiate tx:', res.transactionHash);
  } catch (e) {
    console.error('Instantiate failed:', e.message);
    process.exit(1);
  }
  console.log();

  // --- Step 2.5: Verify admin ---
  console.log('--- Step 2.5: Verify contract admin ---');
  try {
    const resp = await fetch(`${REST_URL}/cosmwasm/wasm/v1/contract/${newMoultbookAddr}`);
    if (resp.ok) {
      const data = await resp.json();
      const admin = data.contract?.contract_info?.admin || data.contract?.admin || '';
      console.log('Contract admin:', admin || '(none)');
      console.log('Our sender:', sender);
      if (admin !== sender) {
        console.log('WARNING: Admin mismatch! Full response:');
        console.log(JSON.stringify(data, null, 2));
      } else {
        console.log('Admin matches sender ✓');
      }
    }
  } catch (e) {
    console.log('Query error:', e.message);
  }
  console.log();

  // --- Step 3: Register new moultbook with FeePay ---
  console.log('--- Step 3: Register moultbook with FeePay ---');
  const registerMsg = {
    typeUrl: '/juno.feepay.v1.MsgRegisterFeePayContract',
    value: {
      senderAddress: sender,
      feePayContract: {
        contractAddress: newMoultbookAddr,
        balance: 0,
        walletLimit: 1000,
      },
    },
  };

  let registered = false;
  try {
    const fee = calculateFee(500000, GasPrice.fromString(GAS_PRICE));
    const txResult = await stargateClient.signAndBroadcast(sender, [registerMsg], fee, 'register feepay');
    console.log('Register tx:', txResult.transactionHash);
    console.log('Code:', txResult.code, txResult.rawLog);
    if (txResult.code === 0) registered = true;
  } catch (e) {
    console.log('Register result:', e.message);
  }
  if (!registered) {
    console.error('  FAILED — cannot continue without registration');
    process.exit(1);
  }
  console.log();

  // --- Step 4: Fund the FeePay pool ---
  console.log('--- Step 4: Fund FeePay pool with 1,000,000 ujunox ---');
  const fundMsg = {
    typeUrl: '/juno.feepay.v1.MsgFundFeePayContract',
    value: {
      senderAddress: sender,
      contractAddress: newMoultbookAddr,
      amount: [{ denom: 'ujunox', amount: '1000000' }],
    },
  };

  let funded = false;
  try {
    const fee = calculateFee(500000, GasPrice.fromString(GAS_PRICE));
    const txResult = await stargateClient.signAndBroadcast(sender, [fundMsg], fee, 'fund feepay');
    console.log('Fund tx:', txResult.transactionHash);
    console.log('Code:', txResult.code, txResult.rawLog);
    if (txResult.code === 0) funded = true;
  } catch (e) {
    console.log('Fund result:', e.message);
  }
  console.log();

  // --- Step 5: Query pool balance ---
  console.log('--- Step 5: Query FeePay pool balance ---');
  let poolBalanceBefore = null;
  try {
    const resp = await fetch(`${REST_URL}/juno/feepay/v1/contract/${newMoultbookAddr}`);
    if (resp.ok) {
      const data = await resp.json();
      console.log('Pool data:', JSON.stringify(data, null, 2));
      poolBalanceBefore = data.fee_pay_contract?.balance || data.feePayContract?.balance || data.balance;
    } else {
      console.log('Query returned HTTP', resp.status);
      const text = await resp.text();
      console.log('Body:', text);
    }
  } catch (e) {
    console.log('Query error:', e.message);
  }
  console.log();

  // --- Step 6: Send a NORMAL tx (with fees) to moultbook ---
  console.log('--- Step 6: Send normal tx (with fees) to moultbook ---');
  const postMsg = {
    post: {
      commitment: crypto.randomBytes(32).toString('base64'),
      content_type: 'text/plain',
      size_bytes: 32,
      attestation_ref: null,
      visibility: 'public',
      refs: [],
    },
  };

  let senderBalanceBefore = await cosmwasmClient.getBalance(sender, 'ujunox');
  console.log('Sender balance before:', senderBalanceBefore.amount, 'ujunox');

  try {
    const fee = calculateFee(300000, GasPrice.fromString(GAS_PRICE));
    const txResult = await cosmwasmClient.execute(sender, newMoultbookAddr, postMsg, fee);
    console.log('Normal tx:', txResult.transactionHash);
    console.log('Gas used:', txResult.gasUsed);
  } catch (e) {
    console.log('Normal tx result:', e.message);
  }

  let senderBalanceAfter = await cosmwasmClient.getBalance(sender, 'ujunox');
  console.log('Sender balance after:', senderBalanceAfter.amount, 'ujunox');
  console.log('Cost (gas):', BigInt(senderBalanceBefore.amount) - BigInt(senderBalanceAfter.amount), 'ujunox');
  console.log();

  // --- Step 7: Send a GASLESS tx (fees=0) to moultbook ---
  console.log('--- Step 7: Send GASLESS tx (fees=0) to moultbook ---');
  const gaslessPostMsg = {
    post: {
      commitment: crypto.randomBytes(32).toString('base64'),
      content_type: 'text/plain',
      size_bytes: 32,
      attestation_ref: null,
      visibility: 'public',
      refs: [],
    },
  };

  senderBalanceBefore = await cosmwasmClient.getBalance(sender, 'ujunox');
  console.log('Sender balance before:', senderBalanceBefore.amount, 'ujunox');

  try {
    const zeroFee = {
      amount: coins(0, 'ujunox'),
      gas: '300000',
    };
    const txResult = await cosmwasmClient.execute(sender, newMoultbookAddr, gaslessPostMsg, zeroFee);
    console.log('Gasless tx:', txResult.transactionHash);
    console.log('Gas used:', txResult.gasUsed);
    console.log('SUCCESS: FeePay covered the gas!');
  } catch (e) {
    console.log('Gasless tx result:', e.message);
    if (e.message.includes('insufficient') || e.message.includes('fee')) {
      console.log('  FeePay may not have covered the tx. Check if GlobalFee blocks zero-fee txs.');
    }
  }

  senderBalanceAfter = await cosmwasmClient.getBalance(sender, 'ujunox');
  console.log('Sender balance after:', senderBalanceAfter.amount, 'ujunox');
  const senderCost = BigInt(senderBalanceBefore.amount) - BigInt(senderBalanceAfter.amount);
  console.log('Sender cost (should be 0 if FeePay worked):', senderCost, 'ujunox');

  // Query pool balance after gasless tx
  try {
    const resp = await fetch(`${REST_URL}/juno/feepay/v1/contract/${newMoultbookAddr}`);
    if (resp.ok) {
      const data = await resp.json();
      const poolBalanceAfter = data.fee_pay_contract?.balance || data.feePayContract?.balance || data.balance;
      console.log('Pool balance after:', poolBalanceAfter);
      if (poolBalanceBefore && poolBalanceAfter) {
        const poolDiff = BigInt(poolBalanceBefore) - BigInt(poolBalanceAfter);
        console.log('Pool deducted:', poolDiff, 'ujunox');
      }
    }
  } catch (e) {}
  console.log();

  // --- Step 8: Summary ---
  console.log('=== FeePay Test Summary ===');
  console.log('New moultbook:', newMoultbookAddr);
  console.log('1. FeePay module: ENABLED on uni-7');
  console.log('2. Moultbook instantiated with admin: check above');
  console.log('3. Registered with FeePay: ' + (registered ? 'YES' : 'NO'));
  console.log('4. Pool funded: ' + (funded ? 'YES' : 'NO'));
  console.log('5. Normal tx (with fees): check above');
  console.log('6. Gasless tx (fees=0): check above');
  console.log();
  console.log('If the gasless tx succeeded and sender cost = 0, FeePay works on v30!');
  console.log('If it failed, possible causes:');
  console.log('  - GlobalFee floor blocks zero-fee txs before FeePay ante handler');
  console.log('  - FeePay ante handler ordering issue (needs v31 fix)');
  console.log();

  // Save the new contract address
  deployed['feepay-test-moultbook'] = { codeId: moultbookCodeId, address: newMoultbookAddr };
  writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  console.log('Saved feepay-test-moultbook address to deployed-testnet.json');
}

main().catch(console.error);
