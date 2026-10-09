//! UniFFI boundary selected by Wcash mobile builds.
//!
//! This crate exports the upstream function names so the native and React
//! layers keep their exact interface. Every wallet operation either routes to
//! `wcash-mobile-adapter` or returns an explicit unsupported-Wcash error. It
//! has no dependency on the upstream Zcash `LightClient` FFI crate.

uniffi::include_scaffolding!("zingo");

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use once_cell::sync::Lazy;
use wcash_mobile_adapter::{
    AdapterError, MobileNetwork, WalletSyncCancellation, WcashMobileAdapter,
    read_wallet_recovery_info as adapter_read_recovery,
    validate_wallet_bytes as adapter_validate_wallet,
};

const WALLET_DATABASE_NAME: &str = "wcash-wallet.sqlite";
const WALLET_FORMAT_VERSION: u64 = 700;
const HISTORY_LIMIT: usize = 50;

static SESSION: Lazy<Mutex<Option<WcashMobileAdapter>>> = Lazy::new(|| Mutex::new(None));
static WALLET_DIRECTORY: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));
static SYNC_CANCELLATION: Lazy<RwLock<Option<WalletSyncCancellation>>> =
    Lazy::new(|| RwLock::new(None));
static SYNC_RUNNING: AtomicBool = AtomicBool::new(false);
static LAST_SYNC_POLL: Lazy<RwLock<Option<Result<String, String>>>> =
    Lazy::new(|| RwLock::new(None));
static LAST_SYNC_STATUS: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));
static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Runtime::new().expect("the Wcash mobile Tokio runtime must initialize")
});

#[derive(Debug, thiserror::Error)]
pub enum ZingolibError {
    #[error("Error: Wcash wallet is not initialized")]
    LightclientNotInitialized,
    #[error("Error: Wcash wallet lock poisoned")]
    LightclientLockPoisoned,
    #[error("Error: panic: {0}")]
    Panic(String),
    #[error("Error: saving Wcash wallet: {0}")]
    Save(String),
    #[error("Error: initializing Wcash wallet: {0}")]
    Init(String),
    #[error("Error: Wcash sync: {0}")]
    Sync(String),
    #[error("Error: Wcash rescan: {0}")]
    Rescan(String),
    #[error("Error: reading Wcash wallet: {0}")]
    Read(String),
    #[error("Error: Wcash send: {0}")]
    Send(String),
    #[error("Error: Wcash shield: {0}")]
    Shield(String),
    #[error("Error: invalid input: {0}")]
    InvalidInput(String),
    #[error("Error: Wcash wallet: {0}")]
    Wallet(String),
    #[error("Error: Wcash endpoint: {0}")]
    Indexer(String),
    #[error("Error: offline Wcash mode is not implemented")]
    Offline,
    #[error("Error: Wcash progress channel lock poisoned")]
    SideChannelPoisoned,
    #[error("Error: unsupported Wcash feature: migration is not in progress")]
    MigrationNotInProgress,
    #[error("Error: unsupported Wcash feature: migration is already in progress")]
    MigrationAlreadyInProgress,
    #[error("Error: unsupported Wcash feature: stale migration consent: {0}")]
    MigrationConsentStale(String),
    #[error("Error: unsupported Wcash feature: fixed migration cadence: {0}")]
    MigrationCadenceFixed(String),
    #[error("Error: unsupported Wcash feature: migration split: {0}")]
    MigrationSplit(String),
    #[error("Error: unsupported Wcash feature: migration: {0}")]
    Migration(String),
    #[error("Error: unsupported Wcash feature: mixnet: {0}")]
    Mixnet(String),
}

fn unsupported(feature: &str) -> ZingolibError {
    ZingolibError::InvalidInput(format!("unsupported Wcash feature: {feature}"))
}

fn map_adapter(error: AdapterError) -> ZingolibError {
    let message = error.to_string();
    match error {
        AdapterError::RegtestDisabled
        | AdapterError::InvalidInput(_)
        | AdapterError::InvalidSeed
        | AdapterError::InvalidWalletBytes(_)
        | AdapterError::Json(_) => ZingolibError::InvalidInput(message),
        AdapterError::Io(_) | AdapterError::Database(_) => ZingolibError::Save(message),
        AdapterError::SendAlreadyStaged | AdapterError::NoStagedSend => {
            ZingolibError::Send(message)
        }
        AdapterError::UnsupportedFeature(feature) => unsupported(feature),
        AdapterError::Core(_) => ZingolibError::Wallet(message),
    }
}

fn with_session<T>(
    action: impl FnOnce(&WcashMobileAdapter) -> Result<T, AdapterError>,
) -> Result<T, ZingolibError> {
    let session = SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
    let adapter = session
        .as_ref()
        .ok_or(ZingolibError::LightclientNotInitialized)?;
    action(adapter).map_err(map_adapter)
}

fn configured_wallet_path() -> Result<PathBuf, ZingolibError> {
    let directory = WALLET_DIRECTORY
        .read()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
    directory
        .as_ref()
        .map(|path| path.join(WALLET_DATABASE_NAME))
        .ok_or_else(|| {
            ZingolibError::Init(
                "native bridge did not configure the Wcash wallet directory".to_owned(),
            )
        })
}

fn validate_connection_inputs(
    endpoint: &str,
    chain_hint: &str,
    performance_level: &str,
    min_confirmations: u32,
) -> Result<MobileNetwork, ZingolibError> {
    if endpoint.trim().is_empty() {
        return Err(ZingolibError::Offline);
    }
    if !matches!(performance_level, "Low" | "Medium" | "High" | "Maximum") {
        return Err(ZingolibError::InvalidInput(
            "invalid performance level".to_owned(),
        ));
    }
    if min_confirmations == 0 {
        return Err(ZingolibError::InvalidInput(
            "min_confirmations must be greater than zero".to_owned(),
        ));
    }
    MobileNetwork::from_chain_hint(chain_hint).map_err(map_adapter)
}

fn reset_database(path: &Path) -> Result<(), ZingolibError> {
    *SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = None;
    for candidate in [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ZingolibError::Save(error.to_string())),
        }
    }
    Ok(())
}

pub fn set_wallet_directory(directory: String) -> Result<String, ZingolibError> {
    let path = PathBuf::from(directory);
    if !path.is_absolute() {
        return Err(ZingolibError::InvalidInput(
            "wallet directory must be absolute".to_owned(),
        ));
    }
    fs::create_dir_all(&path).map_err(|error| ZingolibError::Save(error.to_string()))?;
    *WALLET_DIRECTORY
        .write()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = Some(path);
    Ok("Wcash adapter selected".to_owned())
}

pub fn init_logging() -> Result<String, ZingolibError> {
    Ok("OK".to_owned())
}

pub fn get_wallet_activation_height(chain_hint: String) -> Result<String, ZingolibError> {
    let network = MobileNetwork::from_chain_hint(&chain_hint).map_err(map_adapter)?;
    Ok(network.wallet_activation_height().to_string())
}

pub fn set_broadcast_candidates(_candidates_json: String) -> Result<String, ZingolibError> {
    Err(unsupported("alternate broadcast candidates"))
}

pub fn init_new(
    server_uri: String,
    _birthday: u32,
    chain_hint: String,
    performance_level: String,
    min_confirmations: u32,
) -> Result<String, ZingolibError> {
    let network = validate_connection_inputs(
        &server_uri,
        &chain_hint,
        &performance_level,
        min_confirmations,
    )?;
    let path = configured_wallet_path()?;
    reset_database(&path)?;
    let (adapter, recovery) = RT
        .block_on(WcashMobileAdapter::create_new(&server_uri, &path, network))
        .map_err(map_adapter)?;
    *SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = Some(adapter);
    Ok(recovery)
}

pub fn init_from_seed(
    seed: String,
    birthday: u32,
    server_uri: String,
    chain_hint: String,
    performance_level: String,
    min_confirmations: u32,
) -> Result<String, ZingolibError> {
    let network = validate_connection_inputs(
        &server_uri,
        &chain_hint,
        &performance_level,
        min_confirmations,
    )?;
    let path = configured_wallet_path()?;
    reset_database(&path)?;
    let (adapter, recovery) = RT
        .block_on(WcashMobileAdapter::restore_from_seed(
            &seed,
            birthday,
            &server_uri,
            &path,
            network,
        ))
        .map_err(map_adapter)?;
    *SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = Some(adapter);
    Ok(recovery)
}

pub fn init_from_ufvk(
    _ufvk: String,
    _birthday: u32,
    _server_uri: String,
    _chain_hint: String,
    _performance_level: String,
    _min_confirmations: u32,
) -> Result<String, ZingolibError> {
    Err(unsupported("UFVK/watch-only restore"))
}

pub fn init_from_bytes(
    wallet_bytes: Vec<u8>,
    server_uri: String,
    chain_hint: String,
    performance_level: String,
    min_confirmations: u32,
) -> Result<String, ZingolibError> {
    let network = validate_connection_inputs(
        &server_uri,
        &chain_hint,
        &performance_level,
        min_confirmations,
    )?;
    let path = configured_wallet_path()?;
    let previous = SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)?
        .take();
    let opened = RT.block_on(WcashMobileAdapter::open_wallet_bytes(
        &wallet_bytes,
        &server_uri,
        &path,
        network,
    ));
    let adapter = match opened {
        Ok(adapter) => adapter,
        Err(error) => {
            *SESSION
                .lock()
                .map_err(|_| ZingolibError::LightclientLockPoisoned)? = previous;
            return Err(map_adapter(error));
        }
    };
    let recovery = adapter.recovery_json().map_err(map_adapter)?;
    *SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = Some(adapter);
    Ok(recovery)
}

pub fn save_wallet_bytes() -> Result<Option<Vec<u8>>, ZingolibError> {
    with_session(|adapter| adapter.save_wallet_bytes().map(Some))
}

pub fn read_wallet_recovery_info(wallet_bytes: Vec<u8>) -> Result<String, ZingolibError> {
    adapter_read_recovery(&wallet_bytes).map_err(map_adapter)
}

pub fn validate_wallet_bytes(wallet_bytes: Vec<u8>) -> Result<(), ZingolibError> {
    adapter_validate_wallet(&wallet_bytes).map_err(map_adapter)
}

pub fn get_latest_block_server(server_uri: String) -> Result<String, ZingolibError> {
    RT.block_on(WcashMobileAdapter::latest_server_height(&server_uri))
        .map(|height| height.to_string())
        .map_err(map_adapter)
}

pub fn get_latest_block_wallet() -> Result<String, ZingolibError> {
    with_session(WcashMobileAdapter::latest_wallet_block_json)
}

pub fn get_developer_donation_address() -> Result<String, ZingolibError> {
    Ok(String::new())
}

pub fn get_zennies_for_zingo_donation_address() -> Result<String, ZingolibError> {
    Ok(String::new())
}

pub fn get_value_transfers() -> Result<String, ZingolibError> {
    with_session(|adapter| adapter.confirmed_history_json(HISTORY_LIMIT))
}

pub fn set_crypto_default_provider_to_ring() -> Result<String, ZingolibError> {
    Ok("OK".to_owned())
}

pub fn poll_sync() -> Result<String, ZingolibError> {
    if SYNC_RUNNING.load(Ordering::Acquire) {
        return Ok("Sync task is not complete".to_owned());
    }
    match LAST_SYNC_POLL
        .write()
        .map_err(|_| ZingolibError::SideChannelPoisoned)?
        .take()
    {
        Some(Ok(poll)) => Ok(poll),
        Some(Err(error)) => Err(ZingolibError::Sync(error)),
        None => Ok("Sync task has not been launched.".to_owned()),
    }
}

struct SyncRunningGuard;

impl Drop for SyncRunningGuard {
    fn drop(&mut self) {
        SYNC_RUNNING.store(false, Ordering::Release);
    }
}

pub fn run_sync() -> Result<String, ZingolibError> {
    if SYNC_RUNNING.swap(true, Ordering::AcqRel) {
        return Ok("Sync task already running.".to_owned());
    }
    let launch = (|| {
        let mut session = SESSION
            .lock()
            .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
        let adapter = session
            .as_mut()
            .ok_or(ZingolibError::LightclientNotInitialized)?;
        let cancellation = adapter.prepare_sync();
        let starting_status = adapter.starting_sync_status_json().map_err(map_adapter)?;
        *SYNC_CANCELLATION
            .write()
            .map_err(|_| ZingolibError::SideChannelPoisoned)? = Some(cancellation);
        *LAST_SYNC_STATUS
            .write()
            .map_err(|_| ZingolibError::SideChannelPoisoned)? = Some(starting_status);
        *LAST_SYNC_POLL
            .write()
            .map_err(|_| ZingolibError::SideChannelPoisoned)? = None;

        std::thread::Builder::new()
            .name("wcash-wallet-sync".to_owned())
            .spawn(|| {
                let _running = SyncRunningGuard;
                let result = (|| {
                    let mut session = SESSION
                        .lock()
                        .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
                    let adapter = session
                        .as_mut()
                        .ok_or(ZingolibError::LightclientNotInitialized)?;
                    let poll = RT.block_on(adapter.synchronize()).map_err(map_adapter)?;
                    let status = adapter.sync_status_json().map_err(map_adapter)?;
                    *LAST_SYNC_STATUS
                        .write()
                        .map_err(|_| ZingolibError::SideChannelPoisoned)? = Some(status);
                    Ok(poll)
                })()
                .map_err(|error: ZingolibError| error.to_string());
                if let Ok(mut latest) = LAST_SYNC_POLL.write() {
                    *latest = Some(result);
                }
            })
            .map(|_| ())
            .map_err(|error| ZingolibError::Sync(error.to_string()))
    })();
    if let Err(error) = launch {
        SYNC_RUNNING.store(false, Ordering::Release);
        return Err(error);
    }
    Ok("Launching sync task...".to_owned())
}

pub fn pause_sync() -> Result<String, ZingolibError> {
    let cancellation = SYNC_CANCELLATION
        .read()
        .map_err(|_| ZingolibError::SideChannelPoisoned)?;
    let cancellation = cancellation
        .as_ref()
        .ok_or_else(|| ZingolibError::Sync("Sync task has not been launched".to_owned()))?;
    cancellation.cancel();
    Ok("Pausing sync task...".to_owned())
}

pub fn status_sync() -> Result<String, ZingolibError> {
    if SYNC_RUNNING.load(Ordering::Acquire) {
        return LAST_SYNC_STATUS
            .read()
            .map_err(|_| ZingolibError::SideChannelPoisoned)?
            .clone()
            .ok_or_else(|| unsupported("live sync progress"));
    }
    with_session(WcashMobileAdapter::sync_status_json)
}

pub fn run_rescan() -> Result<String, ZingolibError> {
    Err(unsupported("rescan"))
}

pub fn info_server() -> Result<String, ZingolibError> {
    let (network, endpoint) =
        with_session(|adapter| Ok((adapter.network(), adapter.endpoint().to_owned())))?;
    RT.block_on(WcashMobileAdapter::server_info_json(&endpoint, network))
        .map_err(map_adapter)
}

pub fn get_seed() -> Result<String, ZingolibError> {
    with_session(WcashMobileAdapter::recovery_json)
}

pub fn get_ufvk() -> Result<String, ZingolibError> {
    Err(unsupported("UFVK export"))
}

pub fn change_server(server_uri: String) -> Result<String, ZingolibError> {
    if server_uri.trim().is_empty() {
        return Err(ZingolibError::Offline);
    }
    let (bytes, network) =
        with_session(|adapter| Ok((adapter.save_wallet_bytes()?, adapter.network())))?;
    let path = configured_wallet_path()?;
    *SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = None;
    let adapter = RT
        .block_on(WcashMobileAdapter::open_wallet_bytes(
            &bytes,
            &server_uri,
            &path,
            network,
        ))
        .map_err(map_adapter)?;
    *SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)? = Some(adapter);
    Ok("server set".to_owned())
}

pub fn wallet_kind() -> Result<String, ZingolibError> {
    with_session(|_| {
        Ok(serde_json::json!({
            "kind": "Loaded from seed or mnemonic phrase",
            "transparent": true,
            "sapling": false,
            // The existing key means the legacy Orchard pool. Wcash accounts
            // are Ironwood-only, so reporting true would expose migration UI.
            "orchard": false
        })
        .to_string())
    })
}

pub fn parse_address(address: String) -> Result<String, ZingolibError> {
    with_session(|adapter| adapter.parse_recipient_json(&address))
}

pub fn parse_ufvk(_ufvk: String) -> Result<String, ZingolibError> {
    Err(unsupported("UFVK parsing"))
}

pub fn get_version() -> Result<String, ZingolibError> {
    Ok(format!(
        "wcash-wallet-core-{}-wcash-mobile-adapter",
        wcash_mobile_adapter::WCASH_WALLET_CORE_REV
    ))
}

pub fn get_messages(_address: String) -> Result<String, ZingolibError> {
    Err(unsupported("memo message history"))
}

pub fn get_balance() -> Result<String, ZingolibError> {
    with_session(WcashMobileAdapter::balance_json)
}

pub fn get_total_memobytes_to_address() -> Result<String, ZingolibError> {
    Err(unsupported("memo analytics"))
}

pub fn get_total_value_to_address() -> Result<String, ZingolibError> {
    Err(unsupported("recipient value analytics"))
}

pub fn get_total_spends_to_address() -> Result<String, ZingolibError> {
    Err(unsupported("recipient spend analytics"))
}

pub fn zec_price() -> Result<String, ZingolibError> {
    Err(unsupported("WEC market price"))
}

pub fn remove_transaction(_txid: String) -> Result<String, ZingolibError> {
    Err(unsupported("transaction removal"))
}

pub fn get_spendable_balance_with_address(
    address: String,
    _zennies: String,
) -> Result<String, ZingolibError> {
    with_session(|adapter| {
        adapter.parse_recipient_json(&address)?;
        adapter.spendable_balance_json()
    })
}

pub fn get_spendable_balance_total() -> Result<String, ZingolibError> {
    with_session(WcashMobileAdapter::spendable_balance_json)
}

pub fn set_option_wallet() -> Result<String, ZingolibError> {
    Err(unsupported("wallet options"))
}

pub fn get_option_wallet() -> Result<String, ZingolibError> {
    Err(unsupported("wallet options"))
}

pub fn get_unified_addresses() -> Result<String, ZingolibError> {
    with_session(WcashMobileAdapter::unified_addresses_json)
}

pub fn get_transparent_addresses() -> Result<String, ZingolibError> {
    with_session(WcashMobileAdapter::transparent_addresses_json)
}

pub fn create_new_unified_address(_receivers: String) -> Result<String, ZingolibError> {
    Err(unsupported("diversified address creation"))
}

pub fn create_new_transparent_address() -> Result<String, ZingolibError> {
    Err(unsupported("additional transparent address creation"))
}

pub fn check_my_address(address: String) -> Result<String, ZingolibError> {
    let unified = with_session(WcashMobileAdapter::unified_addresses_json)?;
    let transparent = with_session(WcashMobileAdapter::transparent_addresses_json)?;
    let owns = [unified, transparent].iter().any(|encoded| {
        serde_json::from_str::<serde_json::Value>(encoded)
            .ok()
            .and_then(|value| value.as_array().cloned())
            .is_some_and(|rows| {
                rows.iter().any(|row| {
                    row.get("encoded_address").and_then(|value| value.as_str())
                        == Some(address.as_str())
                })
            })
    });
    Ok(serde_json::json!({ "is_wallet_address": owns }).to_string())
}

pub fn get_wallet_save_required() -> Result<String, ZingolibError> {
    with_session(|_| Ok(serde_json::json!({ "save_required": true }).to_string()))
}

pub fn set_config_wallet_to_test() -> Result<String, ZingolibError> {
    with_session(|_| Ok("Wcash network policy remains enforced".to_owned()))
}

pub fn set_config_wallet_to_prod(
    performance_level: String,
    min_confirmations: u32,
) -> Result<String, ZingolibError> {
    if !matches!(
        performance_level.as_str(),
        "Low" | "Medium" | "High" | "Maximum"
    ) {
        return Err(ZingolibError::InvalidInput(
            "invalid performance level".to_owned(),
        ));
    }
    if min_confirmations == 0 {
        return Err(ZingolibError::InvalidInput(
            "min_confirmations must be greater than zero".to_owned(),
        ));
    }
    with_session(|_| Ok("Wcash network confirmation policy remains enforced".to_owned()))
}

pub fn get_config_wallet_performance() -> Result<String, ZingolibError> {
    with_session(|_| Ok(serde_json::json!({ "performance_level": "Medium" }).to_string()))
}

pub fn get_wallet_version() -> Result<String, ZingolibError> {
    with_session(|_| {
        Ok(serde_json::json!({
            "current_version": WALLET_FORMAT_VERSION,
            "read_version": WALLET_FORMAT_VERSION
        })
        .to_string())
    })
}

pub fn send(send_json: String) -> Result<String, ZingolibError> {
    let mut session = SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
    let adapter = session
        .as_mut()
        .ok_or(ZingolibError::LightclientNotInitialized)?;
    RT.block_on(adapter.stage_send_json(&send_json))
        .map_err(map_adapter)
}

pub fn shield() -> Result<String, ZingolibError> {
    let mut session = SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
    let adapter = session
        .as_mut()
        .ok_or(ZingolibError::LightclientNotInitialized)?;
    adapter.stage_shield_json().map_err(map_adapter)
}

pub fn confirm() -> Result<String, ZingolibError> {
    let mut session = SESSION
        .lock()
        .map_err(|_| ZingolibError::LightclientLockPoisoned)?;
    let adapter = session
        .as_mut()
        .ok_or(ZingolibError::LightclientNotInitialized)?;
    RT.block_on(adapter.confirm_send_json())
        .map_err(map_adapter)
}

macro_rules! unsupported_string_fn {
    ($name:ident, $label:literal) => {
        pub fn $name() -> Result<String, ZingolibError> {
            Err(unsupported($label))
        }
    };
}

unsupported_string_fn!(plan_orchard_drain, "Orchard migration planning");
unsupported_string_fn!(drain_orchard_to_ironwood, "Orchard migration");
unsupported_string_fn!(drain_status, "Orchard migration progress");
unsupported_string_fn!(plan_ironwood_migration, "private migration planning");

pub fn start_ironwood_migration(
    _plan_hash_hex: String,
    _per_bucket: Option<u32>,
) -> Result<String, ZingolibError> {
    Err(unsupported("private migration"))
}

unsupported_string_fn!(continue_note_splitting, "migration note splitting");
unsupported_string_fn!(quick_split, "migration quick split");
unsupported_string_fn!(split_status, "migration split progress");

pub fn reschedule_parts(_per_bucket: u32) -> Result<String, ZingolibError> {
    Err(unsupported("migration rescheduling"))
}

unsupported_string_fn!(migration_status, "private migration status");
unsupported_string_fn!(window_timeline, "private migration timeline");
unsupported_string_fn!(reconcile_migration, "private migration reconciliation");

pub fn execute_due_parts(_spacing_ms: u64) -> Result<String, ZingolibError> {
    Err(unsupported("private migration execution"))
}

unsupported_string_fn!(
    execute_due_parts_status,
    "private migration execution progress"
);
unsupported_string_fn!(cancel_ironwood_migration, "private migration cancellation");

pub fn attach_mixnet(_socks5_addr: String, _exit_node: String) -> Result<String, ZingolibError> {
    Err(unsupported("mixnet transport"))
}

pub fn enable_mixnet(_proxy_path: String) -> Result<String, ZingolibError> {
    Err(unsupported("mixnet transport"))
}

pub fn disable_mixnet() -> Result<String, ZingolibError> {
    Ok(serde_json::json!({ "mixnet_indicator": "off" }).to_string())
}

unsupported_string_fn!(mixnet_indicator, "mixnet status");
unsupported_string_fn!(mixnet_bootstrap_detail, "mixnet bootstrap detail");

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for_sync_result(timeout: std::time::Duration) -> Result<serde_json::Value, String> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match poll_sync() {
                Ok(value) if value.starts_with("Sync task is not complete") => {
                    if std::time::Instant::now() >= deadline {
                        return Err("Wcash synchronization exceeded its deadline".to_owned());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Ok(value) => {
                    return serde_json::from_str(&value).map_err(|error| error.to_string());
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }

    fn wait_for_sync(timeout: std::time::Duration) -> serde_json::Value {
        wait_for_sync_result(timeout)
            .unwrap_or_else(|error| panic!("local Wcash synchronization failed: {error}"))
    }

    #[test]
    fn unsupported_surfaces_never_look_like_success() {
        let error = get_messages(String::new()).expect_err("memo history is unavailable");
        assert!(error.to_string().contains("unsupported Wcash feature"));
    }

    #[test]
    fn disable_mixnet_reports_the_inactive_transport() {
        assert_eq!(disable_mixnet().unwrap(), r#"{"mixnet_indicator":"off"}"#);
    }

    #[test]
    fn version_reports_the_locked_wcash_wallet_core_revision() {
        assert_eq!(
            get_version().unwrap(),
            format!(
                "wcash-wallet-core-{}-wcash-mobile-adapter",
                wcash_mobile_adapter::WCASH_WALLET_CORE_REV
            )
        );
    }

    #[test]
    fn mainnet_is_selected_and_offline_fails_before_wallet_creation() {
        assert_eq!(
            get_wallet_activation_height("main".to_owned()).unwrap(),
            "1"
        );
        assert_eq!(
            validate_connection_inputs("https://mainnet.zecwec.com:443", "main", "Medium", 3)
                .unwrap(),
            wcash_mobile_adapter::MobileNetwork::Mainnet
        );
        assert!(matches!(
            validate_connection_inputs("", "regtest", "Medium", 1),
            Err(ZingolibError::Offline)
        ));
    }

    #[test]
    #[ignore = "requires the public Wcash Mainnet CompactTxStreamer endpoint"]
    fn public_mainnet_native_boundary_scans_from_genesis() {
        let directory = tempfile::tempdir().unwrap();
        set_wallet_directory(directory.path().to_string_lossy().into_owned()).unwrap();
        let endpoint = "https://mainnet.zecwec.com:443";
        let recovery: serde_json::Value = serde_json::from_str(
            &init_new(
                endpoint.to_owned(),
                0,
                "main".to_owned(),
                "Medium".to_owned(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(recovery["chain_name"], "main");

        let server: serde_json::Value = serde_json::from_str(&info_server().unwrap()).unwrap();
        assert_eq!(server["chain_name"], "main");
        let result = (1..=4)
            .find_map(|attempt| {
                assert_eq!(run_sync().unwrap(), "Launching sync task...");
                match wait_for_sync_result(std::time::Duration::from_secs(900)) {
                    Ok(result) => Some(result),
                    Err(error) if attempt < 4 => {
                        eprintln!("Wcash Mainnet sync attempt {attempt} will resume: {error}");
                        None
                    }
                    Err(error) => panic!("Wcash Mainnet sync did not complete: {error}"),
                }
            })
            .expect("the final Mainnet sync attempt must return or panic");
        let height = result["sync_complete"]["sync_end_height"]
            .as_u64()
            .expect("Mainnet scan height");
        assert!(
            height > 100,
            "expected more than 100 Mainnet blocks, found {height}"
        );
        eprintln!("Wcash Mainnet mobile FFI scanned through block {height}");

        let addresses: serde_json::Value =
            serde_json::from_str(&get_unified_addresses().unwrap()).unwrap();
        let address = addresses[0]["encoded_address"].as_str().unwrap();
        assert!(address.starts_with("wu1"));
        assert!(parse_address(address.to_owned()).is_ok());
        assert!(serde_json::from_str::<serde_json::Value>(&get_balance().unwrap()).is_ok());
        assert!(serde_json::from_str::<serde_json::Value>(&get_value_transfers().unwrap()).is_ok());
        let bytes = save_wallet_bytes().unwrap().unwrap();
        validate_wallet_bytes(bytes.clone()).unwrap();
        let persisted: serde_json::Value =
            serde_json::from_str(&read_wallet_recovery_info(bytes).unwrap()).unwrap();
        assert_eq!(persisted["chain_name"], "main");
    }

    #[test]
    fn sync_launch_without_a_wallet_fails_and_releases_the_launch_guard() {
        *SESSION.lock().unwrap() = None;
        *LAST_SYNC_POLL.write().unwrap() = None;
        SYNC_RUNNING.store(false, Ordering::Release);

        assert!(matches!(
            run_sync(),
            Err(ZingolibError::LightclientNotInitialized)
        ));
        assert!(!SYNC_RUNNING.load(Ordering::Acquire));
        assert_eq!(poll_sync().unwrap(), "Sync task has not been launched.");
    }

    #[test]
    #[ignore = "requires the local Wcash Regtest indexer on 127.0.0.1:48234"]
    fn local_regtest_native_boundary_runs_the_first_slice() {
        let directory = tempfile::tempdir().unwrap();
        set_wallet_directory(directory.path().to_string_lossy().into_owned()).unwrap();
        let endpoint = "http://127.0.0.1:48234";
        let recovery: serde_json::Value = serde_json::from_str(
            &init_new(
                endpoint.to_owned(),
                0,
                "regtest".to_owned(),
                "Medium".to_owned(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(recovery["chain_name"], "regtest");

        let info: serde_json::Value = serde_json::from_str(&info_server().unwrap()).unwrap();
        assert_eq!(info["chain_name"], "regtest");
        assert_eq!(info["vendor"], "Wcash Wallet");
        assert_eq!(run_sync().unwrap(), "Launching sync task...");

        let poll = wait_for_sync(std::time::Duration::from_secs(20));
        assert!(poll["sync_complete"]["sync_end_height"].is_number());
        let status: serde_json::Value = serde_json::from_str(&status_sync().unwrap()).unwrap();
        assert_eq!(status["scan_ranges"][0]["priority"], "Scanned");
        assert!(serde_json::from_str::<serde_json::Value>(&get_balance().unwrap()).is_ok());
        let addresses: serde_json::Value =
            serde_json::from_str(&get_unified_addresses().unwrap()).unwrap();
        let address = addresses[0]["encoded_address"].as_str().unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&get_value_transfers().unwrap()).is_ok());

        assert!(parse_address(address.to_owned()).is_ok());

        let bytes = save_wallet_bytes().unwrap().unwrap();
        validate_wallet_bytes(bytes.clone()).unwrap();
        let persisted_recovery: serde_json::Value =
            serde_json::from_str(&read_wallet_recovery_info(bytes).unwrap()).unwrap();
        assert_eq!(persisted_recovery["chain_name"], "regtest");
    }

    #[test]
    #[ignore = "requires a funded disposable seed plus the local Wcash Regtest miner/indexer"]
    fn local_regtest_funded_shield_send_mine_sync_and_reopen_through_ffi() {
        let endpoint = "http://127.0.0.1:48234";
        let funded_seed = std::env::var("WCASH_MOBILE_FUNDED_TEST_SEED")
            .expect("WCASH_MOBILE_FUNDED_TEST_SEED must contain the disposable Regtest phrase");
        let miner = std::env::var("WCASH_MOBILE_REGTEST_MINER")
            .expect("WCASH_MOBILE_REGTEST_MINER must name the native merge-miner binary");
        let wcash_genesis = std::env::var("WCASH_EXPECTED_GENESIS_HASH")
            .expect("WCASH_EXPECTED_GENESIS_HASH is required");
        let zcash_genesis = std::env::var("ZCASH_EXPECTED_GENESIS_HASH")
            .expect("ZCASH_EXPECTED_GENESIS_HASH is required");
        let wcash_payout =
            std::env::var("WCASH_PAYOUT_ADDRESS").expect("WCASH_PAYOUT_ADDRESS is required");
        let zcash_payout =
            std::env::var("ZCASH_PAYOUT_ADDRESS").expect("ZCASH_PAYOUT_ADDRESS is required");
        let fixture = tempfile::tempdir().unwrap();
        let mine_block = |journal_name: &str| {
            let output = std::process::Command::new(&miner)
                .args([
                    "native-mine",
                    "http://127.0.0.1:48232",
                    "http://127.0.0.1:18232",
                    "http://127.0.0.1:18242",
                    "-",
                    "256",
                    "0",
                ])
                .env("WCASH_EXPECTED_GENESIS_HASH", &wcash_genesis)
                .env("ZCASH_EXPECTED_GENESIS_HASH", &zcash_genesis)
                .env("ZCASH_NETWORK", "regtest")
                .env("WCASH_SHARE_JOURNAL", fixture.path().join(journal_name))
                .env("WCASH_PAYOUT_ADDRESS", &wcash_payout)
                .env("ZCASH_PAYOUT_ADDRESS", &zcash_payout)
                .output()
                .expect("the Regtest merge miner must launch");
            assert!(
                output.status.success(),
                "the Regtest merge miner must submit the confirmation block"
            );
        };

        // Exercise the native create path and obtain a real active-network
        // Ironwood recipient without bypassing the mobile boundary.
        let recipient_directory = fixture.path().join("recipient");
        set_wallet_directory(recipient_directory.to_string_lossy().into_owned()).unwrap();
        let recipient_recovery: serde_json::Value = serde_json::from_str(
            &init_new(
                endpoint.to_owned(),
                0,
                "regtest".to_owned(),
                "Medium".to_owned(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        let recipient_seed = recipient_recovery["seed_phrase"]
            .as_str()
            .unwrap()
            .to_owned();
        run_sync().unwrap();
        wait_for_sync(std::time::Duration::from_secs(30));
        let addresses: serde_json::Value =
            serde_json::from_str(&get_unified_addresses().unwrap()).unwrap();
        let recipient = addresses[0]["encoded_address"].as_str().unwrap().to_owned();
        let parsed: serde_json::Value =
            serde_json::from_str(&parse_address(recipient.clone()).unwrap()).unwrap();
        assert_eq!(parsed["chain_name"], "regtest");

        // Restore the funded fixture through the same FFI call used by the
        // unchanged recovery screen, then sync before constructing a send.
        let sender_directory = fixture.path().join("sender");
        set_wallet_directory(sender_directory.to_string_lossy().into_owned()).unwrap();
        init_from_seed(
            funded_seed,
            1,
            endpoint.to_owned(),
            "regtest".to_owned(),
            "Medium".to_owned(),
            1,
        )
        .unwrap();
        run_sync().unwrap();
        wait_for_sync(std::time::Duration::from_secs(60));
        let balance: serde_json::Value = serde_json::from_str(&get_balance().unwrap()).unwrap();
        assert!(
            balance["confirmed_transparent_balance"]
                .as_u64()
                .is_some_and(|value| value > 10_000),
            "the disposable Regtest fixture must contain mature transparent coinbase funds"
        );

        // The upstream mining-receiver control remains visible, so its exact
        // shield preview, confirmation, restart persistence, and mined history
        // transition are part of the mobile boundary gate.
        let shield_preview: serde_json::Value =
            serde_json::from_str(&shield().expect("mature coinbase must stage")).unwrap();
        assert!(shield_preview["fee"].as_u64().is_some_and(|fee| fee > 0));
        assert!(
            shield_preview["value_to_shield"]
                .as_u64()
                .is_some_and(|value| value > shield_preview["fee"].as_u64().unwrap())
        );
        let repeated_shield: serde_json::Value =
            serde_json::from_str(&shield().expect("identical shield preview must be stable"))
                .unwrap();
        assert_eq!(repeated_shield, shield_preview);
        let shield_result: serde_json::Value =
            serde_json::from_str(&confirm().expect("exact shield proposal must broadcast"))
                .unwrap();
        let shield_txid = shield_result["txids"][0].as_str().unwrap().to_owned();
        assert_eq!(shield_txid.len(), 64);

        let shield_bytes = save_wallet_bytes().unwrap().unwrap();
        let shield_restart_directory = fixture.path().join("restarted-shield");
        set_wallet_directory(shield_restart_directory.to_string_lossy().into_owned()).unwrap();
        init_from_bytes(
            shield_bytes,
            endpoint.to_owned(),
            "regtest".to_owned(),
            "Medium".to_owned(),
            1,
        )
        .unwrap();
        let pending_shield_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            pending_shield_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["txid"] == shield_txid && row["kind"] == "shield")
        );
        mine_block("shield-share-journal.jsonl");
        run_sync().unwrap();
        wait_for_sync(std::time::Duration::from_secs(60));
        let confirmed_shield_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            confirmed_shield_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| {
                    row["txid"] == shield_txid
                        && row["kind"] == "shield"
                        && row["status"] == "confirmed"
                }),
            "shield history did not transition to confirmed: {confirmed_shield_history}"
        );

        let spendable: serde_json::Value =
            serde_json::from_str(&get_spendable_balance_total().unwrap()).unwrap();
        assert!(
            spendable["spendable_balance"].as_u64().unwrap_or_default() > 110_000,
            "the disposable Regtest fixture must contain spendable Ironwood funds"
        );

        let payment = format!(r#"[{{"address":"{recipient}","amount":100000}}]"#);
        let first_preview: serde_json::Value =
            serde_json::from_str(&send(payment.clone()).unwrap()).unwrap();
        assert!(
            first_preview["fee"]
                .as_u64()
                .is_some_and(|fee| fee >= 10_000)
        );
        let repeated_preview: serde_json::Value =
            serde_json::from_str(&send(payment).unwrap()).unwrap();
        assert_eq!(repeated_preview["fee"], first_preview["fee"]);

        let confirmed = confirm().expect("exact consented proposal must broadcast");
        let confirmed: serde_json::Value = serde_json::from_str(&confirmed).unwrap();
        let txid = confirmed["txids"][0].as_str().unwrap().to_owned();
        assert_eq!(txid.len(), 64);

        let pending_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            pending_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| {
                    row["txid"] == txid
                        && matches!(row["status"].as_str(), Some("transmitted" | "calculated"))
                })
        );

        // Simulate an immediate process restart from the exact mobile export
        // before a block is mined. The signed transaction and its pending
        // history projection must survive without constructing a replacement.
        let wallet_bytes = save_wallet_bytes().unwrap().unwrap();
        let restarted_directory = fixture.path().join("restarted");
        set_wallet_directory(restarted_directory.to_string_lossy().into_owned()).unwrap();
        init_from_bytes(
            wallet_bytes,
            endpoint.to_owned(),
            "regtest".to_owned(),
            "Medium".to_owned(),
            1,
        )
        .unwrap();
        let restarted_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            restarted_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["txid"] == txid && row["status"] != "confirmed")
        );

        // Confirm the broadcast through consensus by mining a merged block.
        // The seed stays in this process environment and is never passed to
        // the miner, its output, or the repository.
        mine_block("send-share-journal.jsonl");

        run_sync().unwrap();
        wait_for_sync(std::time::Duration::from_secs(60));
        let history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        let confirmed_row = history["value_transfers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["txid"] == txid)
            .expect("the mined mobile transaction must appear in confirmed history");
        assert_eq!(confirmed_row["status"], "confirmed");
        assert_eq!(confirmed_row["kind"], "sent");
        let confirmed_sender_bytes = save_wallet_bytes().unwrap().unwrap();

        // Restore the recipient from its recovery phrase after confirmation.
        // This proves an incoming Ironwood note is detected from chain data
        // and remains present after the recipient wallet is reopened.
        let restored_recipient_directory = fixture.path().join("restored-recipient");
        set_wallet_directory(restored_recipient_directory.to_string_lossy().into_owned()).unwrap();
        init_from_seed(
            recipient_seed,
            1,
            endpoint.to_owned(),
            "regtest".to_owned(),
            "Medium".to_owned(),
            1,
        )
        .unwrap();
        run_sync().unwrap();
        wait_for_sync(std::time::Duration::from_secs(60));
        let received_balance: serde_json::Value =
            serde_json::from_str(&get_balance().unwrap()).unwrap();
        assert_eq!(received_balance["total_ironwood_balance"], 100_000);
        let received_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            received_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| {
                    row["txid"] == txid && row["kind"] == "received" && row["status"] == "confirmed"
                })
        );

        let recipient_bytes = save_wallet_bytes().unwrap().unwrap();
        let reopened_recipient_directory = fixture.path().join("reopened-recipient");
        set_wallet_directory(reopened_recipient_directory.to_string_lossy().into_owned()).unwrap();
        init_from_bytes(
            recipient_bytes,
            endpoint.to_owned(),
            "regtest".to_owned(),
            "Medium".to_owned(),
            1,
        )
        .unwrap();
        let reopened_recipient_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            reopened_recipient_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["txid"] == txid && row["kind"] == "received")
        );

        // Save and reopen solely through FFI, then prove the confirmed history
        // survives the same wallet-byte lifecycle used by the native apps.
        validate_wallet_bytes(confirmed_sender_bytes.clone()).unwrap();
        let reopened_directory = fixture.path().join("reopened-confirmed");
        set_wallet_directory(reopened_directory.to_string_lossy().into_owned()).unwrap();
        init_from_bytes(
            confirmed_sender_bytes,
            endpoint.to_owned(),
            "regtest".to_owned(),
            "Medium".to_owned(),
            1,
        )
        .unwrap();
        let reopened_history: serde_json::Value =
            serde_json::from_str(&get_value_transfers().unwrap()).unwrap();
        assert!(
            reopened_history["value_transfers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["txid"] == txid && row["status"] == "confirmed")
        );
    }
}
