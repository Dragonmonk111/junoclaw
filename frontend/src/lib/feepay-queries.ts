// ── FeePay module REST queries for uni-7 ──
//
// Queries the Juno FeePay module via REST (LCD) endpoints.
// As of Aug 21, 2026 we verified on uni-7 that FeePay registration,
// funding, and pool accounting all work on v30. Gasless tx is blocked
// by GlobalFee ante handler ordering — v31 fixes it.

export interface FeePayParams {
  enableFeePay: boolean
}

export interface FeePayContractInfo {
  contractAddress: string
  balance: string
  walletLimit: string
}

export interface FeePayRegisteredContract {
  contractAddress: string
}

const REST_URL = 'https://juno-testnet-api.cogwheel.zone'

export async function queryFeePayParams(): Promise<FeePayParams> {
  const resp = await fetch(`${REST_URL}/juno/feepay/v1/params`)
  if (!resp.ok) throw new Error(`FeePay params query failed: ${resp.status}`)
  const data = await resp.json()
  return {
    enableFeePay: Boolean(data.params?.enable_feepay),
  }
}

export async function queryFeePayContract(contractAddress: string): Promise<FeePayContractInfo | null> {
  const resp = await fetch(`${REST_URL}/juno/feepay/v1/contract/${contractAddress}`)
  if (!resp.ok) {
    if (resp.status === 404 || resp.status === 400) return null
    throw new Error(`FeePay contract query failed: ${resp.status}`)
  }
  const data = await resp.json()
  const c = data.fee_pay_contract
  if (!c) return null
  return {
    contractAddress: String(c.contract_address ?? ''),
    balance: String(c.balance ?? '0'),
    walletLimit: String(c.wallet_limit ?? '0'),
  }
}

export async function queryAllFeePayContracts(): Promise<string[]> {
  const resp = await fetch(`${REST_URL}/juno/feepay/v1/contracts`)
  if (!resp.ok) throw new Error(`FeePay contracts list query failed: ${resp.status}`)
  const data = await resp.json()
  const contracts = data.fee_pay_contracts ?? data.contracts ?? []
  return contracts.map((c: { contract_address?: string } | string) =>
    typeof c === 'string' ? c : String(c.contract_address ?? '')
  )
}
