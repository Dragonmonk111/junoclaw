/** Derive deployer pubkey from SHA256("junoclaw-deployer-v1") and compare
 *  against the pubkey stored on-chain. */
import { Sha256, Secp256k1 } from "@cosmjs/crypto";
import { toHex } from "@cosmjs/encoding";

const priv = new Sha256(new TextEncoder().encode("junoclaw-deployer-v1")).digest();
const pair = await Secp256k1.makeKeypair(priv);
console.log("derived:", toHex(Secp256k1.compressPubkey(pair.pubkey)));
console.log("onchain: 02eb9f28bf64860e79422e260780530f6d65730e2793d1698216fa7b73b9fe1488");
