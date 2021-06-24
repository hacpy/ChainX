// Copyright 2019-2020 ChainX Project Authors. Licensed under GPL-3.0.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use sc_chain_spec::ChainSpecExtension;
use sc_service::{ChainType, Properties};

use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};

use pallet_im_online::sr25519::AuthorityId as ImOnlineId;

use chainx_primitives::{AccountId, AssetId, Balance, ReferralId, Signature};
use chainx_runtime::constants::currency::DOLLARS;
use dev_runtime::constants::{currency::DOLLARS as DEV_DOLLARS, time::DAYS as DEV_DAYS};
use xp_assets_registrar::Chain;
use xp_protocol::{NetworkType, PCX, PCX_DECIMALS, X_BTC};
use xpallet_gateway_bitcoin::{BtcParams, BtcTxVerifier};
use xpallet_gateway_common::types::TrusteeInfoConfig;

use crate::genesis::assets::{genesis_assets, init_assets, pcx, AssetParams};
use crate::genesis::bitcoin::{btc_genesis_params, BtcGenesisParams, BtcTrusteeParams};

use chainx_runtime as chainx;
use dev_runtime as dev;
use malan_runtime as malan;

// Note this is the URL for the telemetry server
#[allow(unused)]
const POLKADOT_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[allow(unused)]
const CHAINX_TELEMETRY_URL: &str = "wss://telemetry.chainx.org/submit/";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    /// Block numbers with known hashes.
    pub fork_blocks: sc_client_api::ForkBlocks<chainx_primitives::Block>,
    /// Known bad block hashes.
    pub bad_blocks: sc_client_api::BadBlocks<chainx_primitives::Block>,
}

/// The `ChainSpec` parameterised for the chainx mainnet runtime.
pub type ChainXChainSpec = sc_service::GenericChainSpec<chainx::GenesisConfig, Extensions>;
/// The `ChainSpec` parameterised for the chainx testnet runtime.
pub type DevChainSpec = sc_service::GenericChainSpec<dev::GenesisConfig, Extensions>;
/// The `ChainSpec` parameterised for the chainx development runtime.
pub type MalanChainSpec = sc_service::GenericChainSpec<malan::GenesisConfig, Extensions>;

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

type AuthorityKeysTuple = (
    (AccountId, ReferralId), // (Staking ValidatorId, ReferralId)
    BabeId,
    GrandpaId,
    ImOnlineId,
    AuthorityDiscoveryId,
);

/// Helper function to generate an authority key for babe
pub fn authority_keys_from_seed(seed: &str) -> AuthorityKeysTuple {
    (
        (
            get_account_id_from_seed::<sr25519::Public>(seed),
            seed.as_bytes().to_vec(),
        ),
        get_from_seed::<BabeId>(seed),
        get_from_seed::<GrandpaId>(seed),
        get_from_seed::<ImOnlineId>(seed),
        get_from_seed::<AuthorityDiscoveryId>(seed),
    )
}

#[inline]
fn balance(input: Balance, decimals: u8) -> Balance {
    input * 10_u128.pow(decimals as u32)
}

/// A small macro for generating the info of PCX endowed accounts.
macro_rules! endowed_gen {
    ( $( ($seed:expr, $value:expr), )+ ) => {
        {
            let mut endowed = BTreeMap::new();
            let pcx_id = pcx().0;
            let endowed_info = vec![
                $((get_account_id_from_seed::<sr25519::Public>($seed), balance($value, PCX_DECIMALS)),)+
            ];
            endowed.insert(pcx_id, endowed_info);
            endowed
        }
    }
}

macro_rules! bootnodes {
    ( $( $bootnode:expr, )* ) => {
        vec![
            $($bootnode.to_string().try_into().expect("The bootnode is invalid"),)*
        ]
    }
}

/// Helper function to generate the network properties.
fn as_properties(network: NetworkType) -> Properties {
    json!({
        "ss58Format": network.ss58_addr_format_id(),
        "network": network,
        "tokenDecimals": PCX_DECIMALS,
        "tokenSymbol": "PCX"
    })
    .as_object()
    .expect("network properties generation can not fail; qed")
    .to_owned()
}

pub fn development_config() -> Result<DevChainSpec, String> {
    let wasm_binary =
        dev::WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;

    let endowed_balance = 50 * DEV_DOLLARS;
    let constructor = move || {
        build_genesis(
            wasm_binary,
            vec![authority_keys_from_seed("Alice")],
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("vesting"),
            genesis_assets(),
            endowed_gen![
                ("Alice", endowed_balance),
                ("Bob", endowed_balance),
                ("Alice//stash", endowed_balance),
                ("Bob//stash", endowed_balance),
            ],
            btc_genesis_params(include_str!("res/btc_genesis_params_testnet.json")),
            crate::genesis::bitcoin::local_testnet_trustees(),
        )
    };
    Ok(DevChainSpec::from_genesis(
        "Development",
        "dev",
        ChainType::Development,
        constructor,
        vec![],
        None,
        Some("chainx-dev"),
        Some(as_properties(NetworkType::Testnet)),
        Default::default(),
    ))
}

#[cfg(feature = "runtime-benchmarks")]
pub fn benchmarks_config() -> Result<DevChainSpec, String> {
    let wasm_binary =
        dev::WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;

    let endowed_balance = 50 * DEV_DOLLARS;
    let constructor = move || {
        build_genesis(
            wasm_binary,
            vec![authority_keys_from_seed("Alice")],
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("vesting"),
            genesis_assets(),
            endowed_gen![
                ("Alice", endowed_balance),
                ("Bob", endowed_balance),
                ("Alice//stash", endowed_balance),
                ("Bob//stash", endowed_balance),
            ],
            btc_genesis_params(include_str!("res/btc_genesis_params_benchmarks.json")),
            crate::genesis::bitcoin::benchmarks_trustees(),
        )
    };
    Ok(DevChainSpec::from_genesis(
        "Benchmarks",
        "dev",
        ChainType::Development,
        constructor,
        vec![],
        None,
        Some("chainx-dev"),
        Some(as_properties(NetworkType::Testnet)),
        Default::default(),
    ))
}

pub fn local_testnet_config() -> Result<DevChainSpec, String> {
    let wasm_binary =
        dev::WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;

    let endowed_balance = 50 * DEV_DOLLARS;
    let constructor = move || {
        build_genesis(
            wasm_binary,
            vec![
                authority_keys_from_seed("Alice"),
                authority_keys_from_seed("Bob"),
            ],
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("vesting"),
            genesis_assets(),
            endowed_gen![
                ("Alice", endowed_balance),
                ("Bob", endowed_balance),
                ("Charlie", endowed_balance),
                ("Dave", endowed_balance),
                ("Eve", endowed_balance),
                ("Ferdie", endowed_balance),
                ("Alice//stash", endowed_balance),
                ("Bob//stash", endowed_balance),
                ("Charlie//stash", endowed_balance),
                ("Dave//stash", endowed_balance),
                ("Eve//stash", endowed_balance),
                ("Ferdie//stash", endowed_balance),
            ],
            btc_genesis_params(include_str!("res/btc_genesis_params_testnet.json")),
            crate::genesis::bitcoin::local_testnet_trustees(),
        )
    };
    Ok(DevChainSpec::from_genesis(
        "Local Testnet",
        "local_testnet",
        ChainType::Local,
        constructor,
        vec![],
        None,
        Some("chainx-local-testnet"),
        Some(as_properties(NetworkType::Testnet)),
        Default::default(),
    ))
}

pub fn mainnet_config() -> Result<ChainXChainSpec, String> {
    ChainXChainSpec::from_json_bytes(&include_bytes!("./res/chainx.json")[..])
}

pub fn malan_config() -> Result<MalanChainSpec, String> {
    MalanChainSpec::from_json_bytes(&include_bytes!("./res/malan.json")[..])
}

pub fn fork_config_raw() -> Result<MalanChainSpec, String> {
    use hex_literal::hex;
    use sp_core::crypto::UncheckedInto;

    let wasm_binary =
        malan::WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

    // 5RGu8p3xo8WH44s6HN2dzvNRRrgRMbbGsHeneFF8L9msxJ5n
    let root_key: AccountId =
        hex!["485bf22c979d4a61643f57a2006ff4fb7447a2a8ed905997c5f6b0230f39b860"].into();
    // 5RGu8p3xo8WH44s6HN2dzvNRRrgRMbbGsHeneFF8L9msxJ5n
    let vesting_key: AccountId =
        hex!["485bf22c979d4a61643f57a2006ff4fb7447a2a8ed905997c5f6b0230f39b860"].into();
    // export SECRET="YOUR SECRET"
    // cd scripts/genesis/generate_keys.sh && bash generate_keys.sh
    let initial_authorities: Vec<AuthorityKeysTuple> = vec![
        (
            (
                // 5CcqG82V8GXnxAfR9Htacg2fF4JJk8cyFRFqbb92KAPB9CAZ
                hex!["1880c73bc154852f900b5db6b3ee9d98c9dd39120f9702ded76f07af558b7d53"].into(),
                b"hacpy1".to_vec(),
            ),
            // 5C7kRjxKBUaJg85L6eZ1LcpwX46qMVuhg38nALaBRM6keo2o
            hex!["0252636a2254619db458c1fe40e91ca39a7bb52bf8c99bd8a4efef458360ba0b"]
                .unchecked_into(),
            // 5FrMW6Jya5NqcWDvTgxw9Xvq57ukF8MKJT7u15Akkb7WfcrR
            hex!["a78577fd7eacdf075bd80fb8dcdbc7c745a43bb2e0785a5a2a9cb8ab142cd9b3"]
                .unchecked_into(),
            // 5C7oRLv5b4ujJcUh8sWYsFYALbNtZYWSUB2v6Aq5u3t3ThUo
            hex!["025c76d4c6369a8c8cb9a74dd91c11d233c0b15767359b404d2f4032f7129302"]
                .unchecked_into(),
            // 5DJ89DTfYsjorQMqajiGUHBJet8rx8yBUrpfHQPewkDsj28Z
            hex!["36782cdf9ee4a785e783580c10cfb9642c9ee11571521a20da22fb08de1dc870"]
                .unchecked_into(),
        ),
        (
            (
                // 5GU2wuoPNoNQtkKRC6PTT3y9LMk2jQ1XaZPqsW7ewnyxywbF
                hex!["c2bbd792a03d62c5f917a6ca0ca6c1513201900b90b555885a26cc90cbef2455"].into(),
                b"rjman1".to_vec(),
            ),
            // 5CkcZQyrGV6EeFpvRqMkvVxBhiZNPRzjfYBzTx7G6H8yUF2k
            hex!["1e6ffbb4f23e91fd42374d1f4e71df694645826b5fe523de83010d17a82fe873"]
                .unchecked_into(),
            // 5FjUPbDafmk54uDju1cKccpcsd4y4oF2LN1kMf25yHUBF8vH
            hex!["a245f00894861c4d597ceaf8d195a240f87aabc5d4e7a6b0a8c5087bc9958e5f"]
                .unchecked_into(),
            // 5GTYn9bSmgb3go1Lis92pfQdMzs6QfNtiPgknKh3Gy9BNiXe
            hex!["c25d04e2d13cfbbed3323dbb69cebe52e4a57f4d29a4e0e1fe4c982df124a643"]
                .unchecked_into(),
            // 5GWQfSHM7NgvtGbDRDuUrPB9RexEXSvQE2ZGyC5sfBC1ScaP
            hex!["c48b6f712581ca56eacc992071abf5224c95e955d1285698e6a2fafae429b80a"]
                .unchecked_into(),
        ),
    ];
    let constructor = move || {
        mainnet_genesis(
            &wasm_binary[..],
            initial_authorities.clone(),
            root_key.clone(),
            vesting_key.clone(),
            genesis_assets(),
            btc_genesis_params(include_str!("res/btc_genesis_params_testnet.json")),
            crate::genesis::bitcoin::local_testnet_trustees(),
        )
    };
    Ok(MalanChainSpec::from_genesis(
        "ChainX Fork",
        "chainx-fork",
        ChainType::Live,
        constructor,
        bootnodes![],
        Some(
            sc_service::config::TelemetryEndpoints::new(vec![])
                .expect("ChainX telemetry url is valid; qed"),
        ),
        Some("pcx-fork"),
        Some(as_properties(NetworkType::Testnet)),
        Default::default(),
    ))
}

fn malan_session_keys(
    babe: BabeId,
    grandpa: GrandpaId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> malan::SessionKeys {
    malan::SessionKeys {
        grandpa,
        babe,
        im_online,
        authority_discovery,
    }
}

fn dev_session_keys(
    babe: BabeId,
    grandpa: GrandpaId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> dev::SessionKeys {
    dev::SessionKeys {
        grandpa,
        babe,
        im_online,
        authority_discovery,
    }
}

fn mainnet_genesis(
    wasm_binary: &[u8],
    initial_authorities: Vec<AuthorityKeysTuple>,
    root_key: AccountId,
    vesting_account: AccountId,
    assets: Vec<AssetParams>,
    bitcoin: BtcGenesisParams,
    trustees: Vec<(Chain, TrusteeInfoConfig, Vec<BtcTrusteeParams>)>,
) -> malan::GenesisConfig {
    use hex_literal::hex;

    // 1000 PCX
    const STAKING_LOCKED: Balance = 100_000 * DOLLARS;
    // 100000 PCX
    const ROOT_ENDOWED: Balance = 10_000_000 * DOLLARS;

    let (assets, assets_restrictions) = init_assets(assets);
    let initial_authorities_len = initial_authorities.len();
    let tech_comm_members: Vec<AccountId> = vec![
        // 5DhacpyA2Ykpjx4AUJGbF7qa8tPqFELEVQYXQsxXQSauPb9r
        hex!["485bf22c979d4a61643f57a2006ff4fb7447a2a8ed905997c5f6b0230f39b860"].into(),
        // 5ERJmanyMqD3Ck2UDkXNwxCsceiNHNiy7frdwYnM8Nxt5cbu
        hex!["682ee67d1c6f6c5db7b3f155f6c31ccadcc373a1178d0fd8e1d2391075e8b424"].into(),
        // 5D7F1AJoDwuCvZZKEggeGk2brxYty9mkamUcFHyshYBnbWs3
        hex!["2e2b928d39b7a9c8688509927e17031001fab604557db093ead5069474e0584e"].into(),
        // 5HG5CswZ6X39BYqt8Dc8e4Cn2HieGnnUiG39ddGn2oq5G36W
        hex!["e5d8bb656b124beb40990ef9346c441f888981ec7e0d4c55c9c72c176aec5290"].into(),
    ];
    let mut balances = initial_authorities
        .iter()
        .map(|((validator, _), _, _, _, _)| validator)
        .cloned()
        .map(|validator| (validator, STAKING_LOCKED))
        .collect::<Vec<_>>();
    // 100 PCX to root account for paying the transaction fee.
    balances.push((root_key.clone(), ROOT_ENDOWED));
    balances.push((
        hex!["682ee67d1c6f6c5db7b3f155f6c31ccadcc373a1178d0fd8e1d2391075e8b424"].into(),
        ROOT_ENDOWED,
    ));
    let initial_authorities_endowed = initial_authorities_len as Balance * STAKING_LOCKED;
    let validators = initial_authorities
        .clone()
        .into_iter()
        .map(|((validator, referral_id), _, _, _, _)| (validator, referral_id, STAKING_LOCKED))
        .collect::<Vec<_>>();

    let mut assets_endowed: BTreeMap<AssetId, Vec<(AccountId, Balance)>> = BTreeMap::new();
    assets_endowed.insert(1, balances.clone());

    let btc_genesis_trustees = trustees
        .iter()
        .find_map(|(chain, _, trustee_params)| {
            if *chain == Chain::Bitcoin {
                Some(
                    trustee_params
                        .iter()
                        .map(|i| (i.0).clone())
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        })
        .expect("bitcoin trustees generation can not fail; qed");
    malan::GenesisConfig {
        frame_system: malan::SystemConfig {
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        },
        pallet_babe: malan::BabeConfig {
            authorities: vec![],
            epoch_config: Some(dev::BABE_GENESIS_EPOCH_CONFIG),
        },
        pallet_grandpa: malan::GrandpaConfig {
            authorities: vec![],
        },
        pallet_collective_Instance1: malan::CouncilConfig::default(),
        pallet_collective_Instance2: malan::TechnicalCommitteeConfig {
            members: tech_comm_members,
            phantom: Default::default(),
        },
        pallet_membership_Instance1: Default::default(),
        pallet_democracy: malan::DemocracyConfig::default(),
        pallet_treasury: Default::default(),
        pallet_elections_phragmen: malan::ElectionsConfig::default(),
        pallet_im_online: malan::ImOnlineConfig { keys: vec![] },
        pallet_authority_discovery: malan::AuthorityDiscoveryConfig { keys: vec![] },
        pallet_session: malan::SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        (x.0).0.clone(),
                        (x.0).0.clone(),
                        malan_session_keys(x.1.clone(), x.2.clone(), x.3.clone(), x.4.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        },
        pallet_balances: malan::BalancesConfig { balances },
        pallet_indices: malan::IndicesConfig { indices: vec![] },
        pallet_sudo: malan::SudoConfig { key: root_key },
        xpallet_system: malan::XSystemConfig {
            network_props: NetworkType::Mainnet,
        },
        xpallet_assets_registrar: malan::XAssetsRegistrarConfig { assets },
        xpallet_assets: malan::XAssetsConfig {
            assets_restrictions,
            endowed: assets_endowed,
        },
        xpallet_gateway_common: malan::XGatewayCommonConfig { trustees },
        xpallet_gateway_bitcoin: malan::XGatewayBitcoinConfig {
            genesis_trustees: btc_genesis_trustees,
            network_id: bitcoin.network,
            confirmation_number: bitcoin.confirmation_number,
            genesis_hash: bitcoin.hash(),
            genesis_info: (bitcoin.header(), bitcoin.height),
            params_info: BtcParams::new(
                486604799,            // max_bits
                2 * 60 * 60,          // block_max_future
                2 * 7 * 24 * 60 * 60, // target_timespan_seconds
                10 * 60,              // target_spacing_seconds
                4,                    // retargeting_factor
            ), // retargeting_factor
            btc_withdrawal_fee: 500000,
            max_withdrawal_count: 100,
            verifier: BtcTxVerifier::Recover,
        },
        xpallet_mining_staking: malan::XStakingConfig {
            validators,
            validator_count: initial_authorities_len as u32, // Start mainnet in PoA
            sessions_per_era: 12,
            vesting_account,
            glob_dist_ratio: (12, 88), // (Treasury, X-type Asset and Staking) = (12, 88)
            mining_ratio: (10, 90),    // (Asset Mining, Staking) = (10, 90)
            minimum_penalty: 100 * DOLLARS,
            candidate_requirement: (100 * DOLLARS, 1_000 * DOLLARS), // Minimum value (self_bonded, total_bonded) to be a validator candidate
            ..Default::default()
        },
        xpallet_mining_asset: malan::XMiningAssetConfig {
            claim_restrictions: vec![(X_BTC, (10, DEV_DAYS * 7))],
            mining_power_map: vec![(X_BTC, 400)],
        },
        xpallet_dex_spot: malan::XSpotConfig {
            trading_pairs: vec![(PCX, X_BTC, 9, 2, 100000, true)],
        },
        xpallet_genesis_builder: malan::XGenesisBuilderConfig {
            params: crate::genesis::genesis_builder_params(),
            root_endowed: ROOT_ENDOWED,
            initial_authorities_endowed,
        },
    }
}

fn build_genesis(
    wasm_binary: &[u8],
    initial_authorities: Vec<AuthorityKeysTuple>,
    root_key: AccountId,
    vesting_account: AccountId,
    assets: Vec<AssetParams>,
    endowed: BTreeMap<AssetId, Vec<(AccountId, Balance)>>,
    bitcoin: BtcGenesisParams,
    trustees: Vec<(Chain, TrusteeInfoConfig, Vec<BtcTrusteeParams>)>,
) -> dev::GenesisConfig {
    const ENDOWMENT: Balance = 10_000_000 * DEV_DOLLARS;
    const STASH: Balance = 100 * DEV_DOLLARS;
    const STAKING_LOCKED: Balance = 1_000 * DEV_DOLLARS;
    let (assets, assets_restrictions) = init_assets(assets);

    let endowed_accounts = endowed
        .get(&PCX)
        .expect("PCX endowed; qed")
        .iter()
        .cloned()
        .map(|(k, _)| k)
        .collect::<Vec<_>>();

    let num_endowed_accounts = endowed_accounts.len();

    let mut total_endowed = Balance::default();
    let balances = endowed
        .get(&PCX)
        .expect("PCX endowed; qed")
        .iter()
        .cloned()
        .map(|(k, _)| {
            total_endowed += ENDOWMENT;
            (k, ENDOWMENT)
        })
        .collect::<Vec<_>>();

    // The value of STASH balance will be reserved per phragmen member.
    let phragmen_members = endowed_accounts
        .iter()
        .take((num_endowed_accounts + 1) / 2)
        .cloned()
        .map(|member| (member, STASH))
        .collect();

    let tech_comm_members = endowed_accounts
        .iter()
        .take((num_endowed_accounts + 1) / 2)
        .cloned()
        .collect::<Vec<_>>();

    // PCX only reserves the native asset id in assets module,
    // the actual native fund management is handled by pallet_balances.
    let mut assets_endowed = endowed;
    assets_endowed.remove(&PCX);

    let mut initial_authorities_endowed = Balance::default();
    let validators = initial_authorities
        .clone()
        .into_iter()
        .map(|((validator, referral), _, _, _, _)| {
            initial_authorities_endowed += STAKING_LOCKED;
            (validator, referral, STAKING_LOCKED)
        })
        .collect::<Vec<_>>();
    let btc_genesis_trustees = trustees
        .iter()
        .find_map(|(chain, _, trustee_params)| {
            if *chain == Chain::Bitcoin {
                Some(
                    trustee_params
                        .iter()
                        .map(|i| (i.0).clone())
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        })
        .expect("bitcoin trustees generation can not fail; qed");

    dev::GenesisConfig {
        frame_system: dev::SystemConfig {
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        },
        pallet_babe: dev::BabeConfig {
            authorities: vec![],
            epoch_config: Some(dev::BABE_GENESIS_EPOCH_CONFIG),
        },
        pallet_grandpa: dev::GrandpaConfig {
            authorities: vec![],
        },
        pallet_collective_Instance1: dev::CouncilConfig::default(),
        pallet_collective_Instance2: dev::TechnicalCommitteeConfig {
            members: tech_comm_members,
            phantom: Default::default(),
        },
        pallet_membership_Instance1: Default::default(),
        pallet_democracy: dev::DemocracyConfig::default(),
        pallet_treasury: Default::default(),
        pallet_elections_phragmen: dev::ElectionsConfig {
            members: phragmen_members,
        },
        pallet_im_online: dev::ImOnlineConfig { keys: vec![] },
        pallet_authority_discovery: dev::AuthorityDiscoveryConfig { keys: vec![] },
        pallet_session: dev::SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        (x.0).0.clone(),
                        (x.0).0.clone(),
                        dev_session_keys(x.1.clone(), x.2.clone(), x.3.clone(), x.4.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        },
        pallet_balances: dev::BalancesConfig { balances },
        pallet_indices: dev::IndicesConfig { indices: vec![] },
        pallet_sudo: dev::SudoConfig { key: root_key },
        xpallet_system: dev::XSystemConfig {
            network_props: NetworkType::Testnet,
        },
        xpallet_assets_registrar: dev::XAssetsRegistrarConfig { assets },
        xpallet_assets: dev::XAssetsConfig {
            assets_restrictions,
            endowed: assets_endowed,
        },
        xpallet_gateway_common: dev::XGatewayCommonConfig { trustees },
        xpallet_gateway_bitcoin: dev::XGatewayBitcoinConfig {
            genesis_trustees: btc_genesis_trustees,
            network_id: bitcoin.network,
            confirmation_number: bitcoin.confirmation_number,
            genesis_hash: bitcoin.hash(),
            genesis_info: (bitcoin.header(), bitcoin.height),
            params_info: BtcParams::new(
                486604799,            // max_bits
                2 * 60 * 60,          // block_max_future
                2 * 7 * 24 * 60 * 60, // target_timespan_seconds
                10 * 60,              // target_spacing_seconds
                4,                    // retargeting_factor
            ), // retargeting_factor
            btc_withdrawal_fee: 500000,
            max_withdrawal_count: 100,
            verifier: BtcTxVerifier::Recover,
        },
        xpallet_mining_staking: dev::XStakingConfig {
            validators,
            validator_count: 50,
            sessions_per_era: 12,
            vesting_account,
            glob_dist_ratio: (12, 88), // (Treasury, X-type Asset and Staking) = (12, 88)
            mining_ratio: (10, 90),    // (Asset Mining, Staking) = (10, 90)
            minimum_penalty: 2 * DOLLARS,
            ..Default::default()
        },
        xpallet_mining_asset: dev::XMiningAssetConfig {
            claim_restrictions: vec![(X_BTC, (10, DEV_DAYS * 7))],
            mining_power_map: vec![(X_BTC, 400)],
        },
        xpallet_dex_spot: dev::XSpotConfig {
            trading_pairs: vec![(PCX, X_BTC, 9, 2, 100000, true)],
        },
        xpallet_genesis_builder: dev::XGenesisBuilderConfig {
            params: crate::genesis::genesis_builder_params(),
            initial_authorities_endowed,
            root_endowed: 0,
        },
    }
}
