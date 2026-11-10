import { queryContractState } from "./tools/chain-query.js";

const REG = "juno1u6jg6v726l4ezvhl36djg3cledf5wl7cnav4x8u5vx7f84hryphqmm4tzu";
const VER = "juno12k0sr39erruxqtwhxndkrlmxdl5kgt5vt7lqxaa4gpxypww866rsjz2xpq";
const CHAIN = "junoclaw-g1";
const MODEL = "Qwen/Qwen2.5-0.5B-Instruct";
const WEIGHTS = "499775ce9c4214a7f3557ba81684983251670027afdb12b62340d01a9ae4627c";

const out: Record<string, unknown> = { queried_at: new Date().toISOString(), chain_id: CHAIN };
out["registry_contract"] = REG;
out["verifier_contract"] = VER;
try { out["config"] = await queryContractState(CHAIN, REG, { get_config: {} }); } catch (e) { out["config_error"] = String(e); }
try { out["active_bank"] = await queryContractState(CHAIN, REG, { active_bank: { model_id: MODEL, weights_hash: WEIGHTS } }); } catch (e) { out["active_bank_error"] = String(e); }
try { out["bank_history"] = await queryContractState(CHAIN, REG, { bank_history: { model_id: MODEL } }); } catch (e) { out["bank_history_error"] = String(e); }
const roots: Record<string, unknown> = {};
out["roots"] = roots;
for (const h of [1, 2, 3, 4]) {
  try {
    roots[`batch_${h}`] = await queryContractState(CHAIN, VER, { get_root: { robot_id: "jlens:thoughts", batch_height: h } });
  } catch (e) {
    roots[`batch_${h}`] = `error: ${e}`;
  }
}
console.log(JSON.stringify(out, null, 2));
