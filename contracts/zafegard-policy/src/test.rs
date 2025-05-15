#![cfg(test)]

use std::println;
extern crate std;

use smart_wallet::{Contract as SmartWalletContract};
use smart_wallet_interface::types::{Signer, SignerExpiration, SignerKey, SignerLimits, SignerStorage};
use soroban_sdk::{
    auth::{Context, ContractContext}, 
    symbol_short, 
    testutils::{Address as _, BytesN as _, EnvTestConfig, Ledger as _}, 
    vec, 
    xdr::ToXdr, 
    Address, 
    BytesN, 
    Env, 
    Error as SorobanError, 
    IntoVal, 
    TryIntoVal, 
    Map, 
    String,
    Vec,
    token::{StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient},
};

use crate::{Contract, ContractClient, Error};

// Import AssetStrategySet from common
use common::models::AssetStrategySet;

const MONTH_IN_LEDGERS: u32 = 3600*24*30/5;

fn vault_contract_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../defindex/defindex_vault.optimized.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

mod vault {
    soroban_sdk::contractimport!(file = "../defindex/defindex_vault.optimized.wasm");
    pub type VaultClient<'a> = Client<'a>;
}
pub use vault::VaultClient;

pub(crate) fn get_token_admin_client<'a>(
    e: &Env,
    address: &Address,
) -> SorobanTokenAdminClient<'a> {
    SorobanTokenAdminClient::new(e, address)
}

pub fn create_vault<'a>(
    e: &Env,
    assets: Vec<AssetStrategySet>,
    roles: Map<u32, Address>,
    vault_fee: u32,
    defindex_protocol_receiver: Address,
    defindex_protocol_rate: u32,
    soroswap_router: Address,
    name_symbol: Map<String, String>,
    upgradable: bool,
) -> VaultClient<'a> {
    let vault_address = &e.register(vault::WASM, (
        assets,
        roles,
        vault_fee,
        defindex_protocol_receiver,
        defindex_protocol_rate,
        soroswap_router,
        name_symbol,
        upgradable,
    ));
    let vault = VaultClient::new(e, vault_address);
    vault
}

// Create a vault with default settings and return all roles and asset address
pub fn create_test_vault<'a>(e: &Env) -> (VaultClient<'a>, SorobanTokenClient<'a>, Address, Address, Address, Address) {
    // Create token for the vault
    let token_admin = Address::generate(e);
    let token = create_token_contract(e, &token_admin);
    
    // Create one asset with no strategies
    let mut assets = Vec::new(e);
    let asset = AssetStrategySet {
        address: token.address.clone(),
        strategies: Vec::new(e),
    };
    assets.push_back(asset);
    
    // Create roles
    let mut roles = Map::new(e);
    let emergency_manager = Address::generate(e);
    let vault_fee_receiver = Address::generate(e);
    let manager = Address::generate(e);
    let rebalance_manager = Address::generate(e);
    
    roles.set(0, emergency_manager.clone());
    roles.set(1, vault_fee_receiver.clone());
    roles.set(2, manager.clone());
    roles.set(3, rebalance_manager.clone());
    
    // Create name_symbol map
    let mut name_symbol = Map::new(e);
    name_symbol.set(String::from_str(e, "name"), String::from_str(e, "Test Vault"));
    name_symbol.set(String::from_str(e, "symbol"), String::from_str(e, "TV"));
    
    // Create mock router address (not actually used in most tests)
    let soroswap_router = Address::generate(e);
    
    // Create vault with default parameters
    let vault = create_vault(
        e,
        assets,
        roles,
        0, // vault_fee
        vault_fee_receiver.clone(), // defindex_protocol_receiver (reusing fee receiver)
        0, // defindex_protocol_rate
        soroswap_router,
        name_symbol,
        true, // upgradable
    );

    let token_admin_client = get_token_admin_client(e, &token.address);
    let amount = 10_0000000;
    token_admin_client.mock_all_auths().mint(&manager, &amount);   
    vault.mock_all_auths().deposit(&vec![&e, amount], &vec![&e, amount], &manager, &false);
    
    // Return vault client, token client and all the roles
    (vault, token, emergency_manager, vault_fee_receiver, manager, rebalance_manager)
}


// Create Test Token
pub(crate) fn create_token_contract<'a>(e: &Env, admin: &Address) -> SorobanTokenClient<'a> {
    SorobanTokenClient::new(
        e,
        &e.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    )
}

#[test]
fn save_money_success() {
    let mut env = Env::default();
    env.set_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });

    env.mock_all_auths();
    env.ledger().set_sequence_number(2*MONTH_IN_LEDGERS);

    let (vault, token, emergency_manager, vault_fee_receiver, manager, rebalance_manager) = create_test_vault(&env);
    
    let root_signer = Signer::Ed25519(BytesN::<32>::random(&env), SignerExpiration(None), SignerLimits(None), SignerStorage::Temporary);
    let wallet = env.register(SmartWalletContract, (root_signer, ));

    let automated_savings_address = env.register(Contract, ());
    let automated_savings_client = ContractClient::new(&env, &automated_savings_address);

    let user = Address::generate(&env);
    let user_bytes = address_to_bytes(&env, &user);
    let amount = 100;

    automated_savings_client.init(&wallet);
    automated_savings_client.add_wallet(&user_bytes, &vault.address, &amount);

    let contexts = vec![
        &env,
        Context::Contract(ContractContext {
            contract: vault.address,
            fn_name: symbol_short!("deposit"),
            args: vec![
                &env,
                vec![&env, amount].try_into_val(&env).unwrap(), // amounts
                vec![&env, amount].try_into_val(&env).unwrap(), // min_amounts
                user.to_val(),      // from
                false.into_val(&env),     // claim
            ],
        }),
    ];

    automated_savings_client.policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts);

    let failed_deposit = automated_savings_client.try_policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts);
    assert_eq!(failed_deposit, Err(Ok(SorobanError::from(Error::TooSoon))));

    env.ledger().set_sequence_number(3*MONTH_IN_LEDGERS);

    // Now we test that only can be deposited to vault address, so with any other address should fail
    let contexts = vec![
        &env,
        Context::Contract(ContractContext {
            contract: Address::generate(&env),
            fn_name: symbol_short!("deposit"),
            args: vec![
                &env,
                vec![&env, amount].try_into_val(&env).unwrap(), // amounts
                vec![&env, amount].try_into_val(&env).unwrap(), // min_amounts
                user.to_val(),      // from
            ],
        }),
    ];

    // let failed_deposit = automated_savings_client.try_policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts);
    // assert_eq!(failed_deposit, Err(Ok(SorobanError::from(Error::NotAllowed))));
}


#[test]
fn test_create_vault() {
    let mut env = Env::default();
    env.set_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    
    // Mock all auths before any contract calls
    env.mock_all_auths();
    
    // Create one asset with no strategies
    let token_admin = Address::generate(&env);
    let token = create_token_contract(&env, &token_admin);
    let mut assets = Vec::new(&env);
    let asset = AssetStrategySet {
        address: token.address.clone(),
        strategies: Vec::new(&env),
    };
    assets.push_back(asset);
    
    // Create roles map
    let mut roles = Map::new(&env);
    let manager = Address::generate(&env);
    let emergency_manager = Address::generate(&env);
    let vault_fee_receiver = Address::generate(&env);
    let rebalance_manager = Address::generate(&env);
    
    roles.set(0, emergency_manager);
    roles.set(1, vault_fee_receiver);
    roles.set(2, manager);
    roles.set(3, rebalance_manager);
    
    // Create name_symbol map
    let mut name_symbol = Map::new(&env);
    name_symbol.set(String::from_str(&env, "name"), String::from_str(&env, "Test Vault"));
    name_symbol.set(String::from_str(&env, "symbol"), String::from_str(&env, "TV"));
    
    // Register the vault contract first
    let vault = create_vault(
        &env,
        assets,
        roles,
        2000,
        Address::generate(&env),
        1000,
        Address::generate(&env),
        name_symbol,
        true,
    );
    
    // Now you can safely check the balance
    assert_eq!(vault.balance(&token.address), 0);
}

// #[test]
// fn test_add_and_use() {
//     let mut env = Env::default();

//     env.set_config(EnvTestConfig {
//         capture_snapshot_at_drop: false,
//     });

//     env.ledger().set_sequence_number(10);

//     env.mock_all_auths();

//     let zafegard_address = env.register(Contract, ());
//     let zafegard_client = ContractClient::new(&env, &zafegard_address);

//     let root_signer = Signer::Ed25519(BytesN::<32>::random(&env), SignerExpiration(None), SignerLimits(None), SignerStorage::Temporary);
//     let wallet = env.register(SmartWalletContract, (root_signer, ) );
//     let sac = Address::generate(&env);
//     let user = Address::generate(&env);
//     let user_bytes = address_to_bytes(&env, &user);
//     let interval = 10;
//     let amount = 100;

//     zafegard_client.init(&wallet);

//     zafegard_client.add_wallet(&user_bytes, &sac, &interval, &amount);

//     let contexts = vec![
//         &env,
//         Context::Contract(ContractContext {
//             contract: sac, // SAC
//             fn_name: symbol_short!("transfer"),
//             args: vec![
//                 &env,
//                 user.to_val(),
//                 wallet.to_val(),
//                 100i128.try_into_val(&env).unwrap(),
//             ],
//         }),
//     ];

//     zafegard_client.policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts);

//     assert_eq!(
//         zafegard_client.try_policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts),
//         Err(Ok(SorobanError::from(Error::TooSoon)))
//     );

//     env.ledger().set_sequence_number(20);

//     zafegard_client.policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts);
// }

// #[test]
// fn test_add_and_remove() {
//     let mut env = Env::default();

//     env.set_config(EnvTestConfig {
//         capture_snapshot_at_drop: false,
//     });

//     env.ledger().set_sequence_number(10);

//     env.mock_all_auths();

//     let zafegard_address = env.register(Contract, ());
//     let zafegard_client = ContractClient::new(&env, &zafegard_address);

//     let root_signer = Signer::Ed25519(BytesN::<32>::random(&env), SignerExpiration(None), SignerLimits(None), SignerStorage::Temporary);
//     let wallet = env.register(SmartWalletContract, (root_signer, ));
//     let sac = Address::generate(&env);
//     let user = Address::generate(&env);
//     let user_bytes = address_to_bytes(&env, &user);
//     let interval = 10;
//     let amount = 100;

//     zafegard_client.init(&wallet);

//     zafegard_client.add_wallet(&user_bytes, &sac, &interval, &amount);

//     let contexts = vec![
//         &env,
//         Context::Contract(ContractContext {
//             contract: sac, // SAC
//             fn_name: symbol_short!("transfer"),
//             args: vec![
//                 &env,
//                 user.to_val(),
//                 wallet.to_val(),
//                 100i128.try_into_val(&env).unwrap(),
//             ],
//         }),
//     ];

//     zafegard_client.policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts);

//     env.ledger().set_sequence_number(20);

//     zafegard_client.remove_wallet(&user_bytes);

//     assert_eq!(
//         zafegard_client.try_policy__(&wallet, &SignerKey::Ed25519(user_bytes.clone()), &contexts),
//         Err(Ok(SorobanError::from(Error::NotFound)))
//     );
// }

fn address_to_bytes(env: &Env, address: &Address) -> BytesN<32> {
    let mut address_array = [0; 32];
    let address_bytes = address.to_xdr(env);

    address_bytes
        .slice(address_bytes.len() - 32..)
        .copy_into_slice(&mut address_array);

    BytesN::from_array(env, &address_array)
}
