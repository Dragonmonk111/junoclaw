/** Dump the stored pub_key Any for an account — is it secp256k1 or hybrid? */
import { grpcUnary } from "../utils/grpc-transport.js";
import {
  QueryAccountRequest,
  QueryAccountResponse,
} from "cosmjs-types/cosmos/auth/v1beta1/query.js";

const ENDPOINT = process.argv[2] ?? "51.195.201.74:9090";
const ADDR = process.argv[3] ?? "juno1dz875zg8p78anpjv3f0qt4gu5a3awpjfhtw992";

const req = QueryAccountRequest.encode(
  QueryAccountRequest.fromPartial({ address: ADDR })
).finish();
const resp = await grpcUnary(
  ENDPOINT,
  "/cosmos.auth.v1beta1.Query/Account",
  Buffer.from(req.buffer, req.byteOffset, req.byteLength)
);
const dec = QueryAccountResponse.decode(resp);
if (!dec.account) {
  console.log("no account");
  process.exit(0);
}
console.log("account Any typeUrl:", dec.account.typeUrl);
console.log("value hex:", Buffer.from(dec.account.value).toString("hex"));

// BaseAccount: field3 = pub_key Any {type_url=1, value=2}
// decode raw Any manually: it IS BaseAccount proto — field 3 is pub_key Any
const v = Buffer.from(dec.account.value);
// quick scan for "/cosmos" or "hybrid" strings
const ascii = v.toString("latin1");
const m = ascii.match(/[a-z./_]*(?:secp256k1|ed25519|hybrid|mayo|PubKey)[A-Za-z0-9./_-]*/g);
console.log("embedded type strings:", m);
