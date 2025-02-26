/// <reference path="../.astro/types.d.ts" />

interface ImportMetaEnv {
    readonly PUBLIC_WALLET_WASM_HASH: string
    readonly PUBLIC_ZAFEGARD_POLICY: string
    readonly PUBLIC_ZAFEGARD_POLICY_WASM: string
    readonly PUBLIC_NATIVE: string
    readonly PUBLIC_PASSPHRASE: string
    readonly PUBLIC_RPC_URL: string
    readonly PUBLIC_LAUNCHTUBE_URL: string
    readonly PUBLIC_LAUNCHTUBE_JWT: string
  }
  
  interface ImportMeta {
    readonly env: ImportMetaEnv;
  }