const { readFileSync, existsSync } = require('fs');
const { join } = require('path');
const { DirectSecp256k1HdWallet, coins, Registry } = require('@cosmjs/proto-signing');
const { GasPrice, calculateFee, defaultRegistryTypes } = require('@cosmjs/stargate');
const { SigningCosmWasmClient } = require('@cosmjs/cosmwasm-stargate');
let protobuf;
try { protobuf = require('protobufjs'); } catch (e) {
  console.error('protobufjs not found. Run: cd deploy && npm install protobufjs');
  process.exit(1);
}

const PARLIAMENT_STATE = join(__dirname, '..', 'wavs', 'bridge', 'parliament-state.json');

function loadMnemonic() {
  if (process.env.JUNO_MNEMONIC) return process.env.JUNO_MNEMONIC;
  if (process.env.MNEMONIC) return process.env.MNEMONIC;
  const role = process.env.PARLIAMENT_ROLE || 'The Builder';
  if (!existsSync(PARLIAMENT_STATE)) {
    console.error(`Error: ${PARLIAMENT_STATE} not found`);
    console.error('Set JUNO_MNEMONIC, MNEMONIC, or PARLIAMENT_ROLE env var');
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

const MOULTBOOK_ADDR = 'juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4';

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
  console.log('=== FeePay Test on uni-7 ===\n');
  console.log('Chain:', CHAIN_ID);
  console.log('RPC:', RPC_URL);
  console.log('Moultbook:', MOULTBOOK_ADDR);
  console.log();

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' });
  const [{ address: sender }] = await wallet.getAccounts();
  console.log('Sender:', sender);

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
    registry: feePayRegistry,
  });

  const balance = await client.getBalance(sender, 'ujunox');
  console.log('Balance:', balance.amount, balance.denom);

  if (BigInt(balance.amount) < 5000000n) {
    console.error('ERROR: Need at least 5 JUNOX for FeePay testing (fund + gas).');
    process.exit(1);
  }
  console.log();

  // --- Step 1: Check if FeePay is enabled ---
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

  // --- Step 1.5: Query contract admin ---
  console.log('--- Step 1.5: Query moultbook contract admin ---');
  try {
    const resp = await fetch(`${REST_URL}/cosmwasm/wasm/v1/contract/${MOULTBOOK_ADDR}`);
    if (resp.ok) {
      const data = await resp.json();
      const admin = data.contract?.admin || '';
      console.log('Contract admin:', admin || '(none)');
      console.log('Our sender:', sender);
      if (admin && admin !== sender) {
        console.log('WARNING: Sender is not the contract admin! FeePay registration will fail.');
        console.log('  Admin:', admin);
        console.log('  Sender:', sender);
      }
    } else {
      console.log('Could not query contract info, HTTP', resp.status);
    }
  } catch (e) {
    console.log('Contract query error:', e.message);
  }
  console.log();

  // --- Step 2: Register moultbook with FeePay ---
  console.log('--- Step 2: Register moultbook with FeePay ---');
  const registerMsg = {
    typeUrl: '/juno.feepay.v1.MsgRegisterFeePayContract',
    value: {
      senderAddress: sender,
      feePayContract: {
        contractAddress: MOULTBOOK_ADDR,
        balance: 0,
        walletLimit: 0,
      },
    },
  };

  try {
    const fee = calculateFee(500000, GasPrice.fromString(GAS_PRICE));
    const txResult = await client.signAndBroadcast(sender, [registerMsg], fee, 'register feepay');
    console.log('Register tx:', txResult.transactionHash);
    console.log('Code:', txResult.code, txResult.rawLog);
  } catch (e) {
    console.log('Register result:', e.message);
    if (e.message.includes('already') || e.message.includes('exists')) {
      console.log('  (already registered, continuing)');
    } else {
      console.error('  FAILED — cannot continue without registration');
      process.exit(1);
    }
  }
  console.log();

  // --- Step 3: Fund the FeePay pool ---
  console.log('--- Step 3: Fund FeePay pool with 1,000,000 ujunox ---');
  const fundMsg = {
    typeUrl: '/juno.feepay.v1.MsgFundFeePayContract',
    value: {
      senderAddress: sender,
      contractAddress: MOULTBOOK_ADDR,
      amount: [{ denom: 'ujunox', amount: '1000000' }],
    },
  };

  try {
    const fee = calculateFee(500000, GasPrice.fromString(GAS_PRICE));
    const txResult = await client.signAndBroadcast(sender, [fundMsg], fee, 'fund feepay');
    console.log('Fund tx:', txResult.transactionHash);
    console.log('Code:', txResult.code, txResult.rawLog);
  } catch (e) {
    console.log('Fund result:', e.message);
  }
  console.log();

  // --- Step 4: Query pool balance ---
  console.log('--- Step 4: Query FeePay pool balance ---');
  try {
    const resp = await fetch(`${REST_URL}/juno/feepay/v1/contract/${MOULTBOOK_ADDR}`);
    if (resp.ok) {
      const data = await resp.json();
      console.log('Pool data:', JSON.stringify(data, null, 2));
    } else {
      console.log('Query returned HTTP', resp.status);
      const text = await resp.text();
      console.log('Body:', text);
    }
  } catch (e) {
    console.log('Query error:', e.message);
  }
  console.log();

  // --- Step 5: Send a NORMAL tx (with fees) to moultbook as baseline ---
  console.log('--- Step 5: Send normal tx (with fees) to moultbook ---');
  const postMsg = {
    post: {
      commitment: Buffer.from('feepay-baseline-test').toString('base64'),
      content_type: 'text/plain',
      size_bytes: 20,
      attestation_ref: null,
      visibility: 'public',
      refs: [],
    },
  };

  let senderBalanceBefore = await client.getBalance(sender, 'ujunox');
  console.log('Sender balance before:', senderBalanceBefore.amount, 'ujunox');

  try {
    const fee = calculateFee(300000, GasPrice.fromString(GAS_PRICE));
    const txResult = await client.execute(sender, MOULTBOOK_ADDR, postMsg, fee);
    console.log('Normal tx:', txResult.transactionHash);
    console.log('Gas used:', txResult.gasUsed);
  } catch (e) {
    console.log('Normal tx result:', e.message);
  }

  let senderBalanceAfter = await client.getBalance(sender, 'ujunox');
  console.log('Sender balance after:', senderBalanceAfter.amount, 'ujunox');
  console.log('Cost (gas):', BigInt(senderBalanceBefore.amount) - BigInt(senderBalanceAfter.amount), 'ujunox');
  console.log();

  // --- Step 6: Send a GASLESS tx (fees=0) to moultbook ---
  console.log('--- Step 6: Send GASLESS tx (fees=0) to moultbook ---');
  const gaslessPostMsg = {
    post: {
      commitment: Buffer.from('feepay-gasless-test').toString('base64'),
      content_type: 'text/plain',
      size_bytes: 20,
      attestation_ref: null,
      visibility: 'public',
      refs: [],
    },
  };

  senderBalanceBefore = await client.getBalance(sender, 'ujunox');
  console.log('Sender balance before:', senderBalanceBefore.amount, 'ujunox');

  // Query pool balance before gasless tx
  let poolBalanceBefore = null;
  try {
    const resp = await fetch(`${REST_URL}/juno/feepay/v1/contract/${MOULTBOOK_ADDR}`);
    if (resp.ok) {
      const data = await resp.json();
      poolBalanceBefore = data.feePayContract?.balance || data.balance;
      console.log('Pool balance before:', poolBalanceBefore);
    }
  } catch (e) {}

  try {
    // Send with zero fees — FeePay should cover the gas
    const zeroFee = {
      amount: coins(0, 'ujunox'),
      gas: '300000',
    };
    const txResult = await client.execute(sender, MOULTBOOK_ADDR, gaslessPostMsg, zeroFee);
    console.log('Gasless tx:', txResult.transactionHash);
    console.log('Gas used:', txResult.gasUsed);
    console.log('SUCCESS: FeePay covered the gas!');
  } catch (e) {
    console.log('Gasless tx result:', e.message);
    if (e.message.includes('insufficient') || e.message.includes('fee')) {
      console.log('  FeePay may not have covered the tx. Check if GlobalFee blocks zero-fee txs.');
    }
  }

  senderBalanceAfter = await client.getBalance(sender, 'ujunox');
  console.log('Sender balance after:', senderBalanceAfter.amount, 'ujunox');
  const senderCost = BigInt(senderBalanceBefore.amount) - BigInt(senderBalanceAfter.amount);
  console.log('Sender cost (should be 0 if FeePay worked):', senderCost, 'ujunox');

  // Query pool balance after gasless tx
  try {
    const resp = await fetch(`${REST_URL}/juno/feepay/v1/contract/${MOULTBOOK_ADDR}`);
    if (resp.ok) {
      const data = await resp.json();
      const poolBalanceAfter = data.feePayContract?.balance || data.balance;
      console.log('Pool balance after:', poolBalanceAfter);
      if (poolBalanceBefore && poolBalanceAfter) {
        const poolDiff = BigInt(poolBalanceBefore) - BigInt(poolBalanceAfter);
        console.log('Pool deducted:', poolDiff, 'ujunox');
      }
    }
  } catch (e) {}
  console.log();

  // --- Step 7: Summary ---
  console.log('=== FeePay Test Summary ===');
  console.log('1. FeePay module: ENABLED on uni-7');
  console.log('2. Moultbook registered: check tx log above');
  console.log('3. Pool funded with 1,000,000 ujunox: check tx log above');
  console.log('4. Normal tx (with fees): check tx log above');
  console.log('5. Gasless tx (fees=0): check if FeePay covered the gas');
  console.log();
  console.log('If the gasless tx succeeded and sender cost = 0, FeePay works on v30!');
  console.log('If it failed, possible causes:');
  console.log('  - GlobalFee floor blocks zero-fee txs before FeePay ante handler');
  console.log('  - FeePay ante handler ordering issue (needs v31 fix)');
  console.log('  - Contract not properly registered');
}

main().catch(console.error);
