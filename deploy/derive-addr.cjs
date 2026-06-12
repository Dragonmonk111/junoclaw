const { DirectSecp256k1HdWallet } = require('@cosmjs/proto-signing');

const mnemonic = process.argv[2];
if (!mnemonic) {
  console.error('Usage: node derive-addr.cjs "mnemonic words..."');
  process.exit(1);
}

DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  .then(w => w.getAccounts())
  .then(a => console.log(a[0].address))
  .catch(err => {
    console.error(err.message);
    process.exit(1);
  });
