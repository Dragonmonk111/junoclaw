import { queryContractState } from "./tools/chain-query.js";

const REG = "juno1u6jg6v726l4ezvhl36djg3cledf5wl7cnav4x8u5vx7f84hryphqmm4tzu";
const CHAIN = "junoclaw-g1";
const MODEL = "Qwen/Qwen2.5-0.5B-Instruct";
const WEIGHTS = "499775ce9c4214a7f3557ba81684983251670027afdb12b62340d01a9ae4627c";

const config = await queryContractState(CHAIN, REG, { get_config: {} });
const active = await queryContractState(CHAIN, REG, { active_bank: { model_id: MODEL, weights_hash: WEIGHTS } });
const history = await queryContractState(CHAIN, REG, { bank_history: { model_id: MODEL } });
console.log(JSON.stringify({ queried_at: new Date().toISOString(), contract: REG, chain_id: CHAIN, config, active_bank: active, bank_history: history }, null, 2));
