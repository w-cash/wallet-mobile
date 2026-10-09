//! Wcash backend adapter for the byte-identical Zingo Mobile interface.
//!
//! This crate is deliberately additive. The upstream `rust/lib` crate remains
//! unchanged until every native FFI entry point can be routed without ever
//! falling back to Zcash. The adapter implements the first usable Wcash slice
//! and emits the JSON shapes already consumed by the React Native application.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use bip0039::{Count, English, Mnemonic};
use rusqlite::{Connection, params};
use secrecy::{ExposeSecret, SecretString, SecretVec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wcash_wallet::{
    AttestedWcashClient, COINBASE_SHIELDING_MATURITY, CalculatedTransaction,
    ConfirmedTransactionSummary, ConfirmedTransactionSummaryHistory, MAX_COINBASE_SHIELDING_INPUTS,
    MAX_CONFIRMED_TRANSACTION_HISTORY_SIZE, MAX_PENDING_TRANSACTION_PAGE_SIZE,
    StagedTransactionProposal, TransferRecipient, WalletNetwork,
    active_pending_signed_transactions, broadcast_calculated_transaction,
    calculate_staged_transaction, cancel_staged_transaction, confirmed_transaction_summary_history,
    decode_recipient, inspect_wallet, propose_coinbase_shielding_offline, propose_transfer_offline,
    verify_wallet_seed,
};
pub use zingolib::wcash::WalletSyncCancellation;
#[cfg(feature = "regtest")]
use zingolib::wcash::WcashRegtestRuntime;
use zingolib::wcash::{
    ConfirmedTransactionDirection, ConfirmedTransactionKind, InitializedWallet,
    WalletBalanceSummary, WalletInfo, WcashMainnetRuntime, WcashTestnetRuntime,
    attested_public_client, wallet_activation_height as consensus_wallet_activation_height,
};

const WALLET_FILE_VERSION: u64 = 700;
const WALLET_FILE_MAGIC: &[u8; 8] = b"WCASHM01";
const WALLET_FILE_HEADER_LEN: usize = 8 + 8 + 1 + 4 + 4 + 8 + 32;
const MAX_SEED_PHRASE_BYTES: usize = 512;
const MAX_WALLET_DATABASE_BYTES: usize = 512 * 1024 * 1024;
const MAX_HISTORY_ROWS: usize = MAX_CONFIRMED_TRANSACTION_HISTORY_SIZE;
const TRANSACTION_EXPIRY_DELTA: u32 = 40;
const TRANSACTION_LOCK_BLOCKS: u32 = 40;
const LOCAL_REGTEST_CONFIRMATIONS: u32 = 1;
const MOBILE_PENDING_TABLE: &str = "wcash_mobile_pending_transactions";

/// Exact wallet-core revision selected by the locked `zingolib` dependency.
pub const WCASH_WALLET_CORE_REV: &str = env!("WCASH_WALLET_CORE_REV");

/// Networks implemented by the reviewed Wcash backend.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MobileNetwork {
    /// Public Wcash Mainnet.
    Mainnet,
    /// Public Wcash Testnet.
    Testnet,
    /// Local Wcash Regtest used by QA builds.
    Regtest,
}

impl MobileNetwork {
    /// Parses the exact short chain token already used by Zingo Mobile.
    pub fn from_chain_hint(chain_hint: &str) -> Result<Self, AdapterError> {
        match chain_hint {
            "test" => Ok(Self::Testnet),
            "regtest" => Ok(Self::Regtest),
            "main" => Ok(Self::Mainnet),
            _ => Err(AdapterError::InvalidInput("unknown chain hint".to_owned())),
        }
    }

    /// Returns the short chain token expected by the React Native app.
    pub const fn chain_name(self) -> &'static str {
        match self {
            Self::Mainnet => "main",
            Self::Testnet => "test",
            Self::Regtest => "regtest",
        }
    }

    /// Returns the ticker of the selected Wcash network.
    pub const fn ticker(self) -> &'static str {
        match self {
            Self::Mainnet => "WEC",
            Self::Testnet | Self::Regtest => "TWC",
        }
    }

    fn wallet_network(self) -> WalletNetwork {
        match self {
            Self::Mainnet => WalletNetwork::Mainnet,
            Self::Testnet => WalletNetwork::Testnet,
            Self::Regtest => WalletNetwork::Regtest,
        }
    }

    /// Returns the wallet activation floor from the selected Wcash consensus parameters.
    pub fn wallet_activation_height(self) -> u32 {
        consensus_wallet_activation_height(self.wallet_network())
    }

    fn envelope_byte(self) -> u8 {
        match self {
            Self::Mainnet => 3,
            Self::Testnet => 1,
            Self::Regtest => 2,
        }
    }

    fn from_envelope_byte(value: u8) -> Result<Self, AdapterError> {
        match value {
            3 => Ok(Self::Mainnet),
            1 => Ok(Self::Testnet),
            2 => Ok(Self::Regtest),
            _ => Err(AdapterError::InvalidWalletBytes(
                "unknown Wcash network discriminator".to_owned(),
            )),
        }
    }
}

/// Stable errors for the future UniFFI boundary.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// Regtest was requested from a build that excludes the local QA backend.
    #[error("Wcash Regtest is not enabled in this build")]
    RegtestDisabled,
    /// An upstream feature has no Wcash backend contract yet.
    #[error("unsupported Wcash feature: {0}")]
    UnsupportedFeature(&'static str),
    /// User input was malformed or outside the supported range.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// A seed phrase was not a canonical 24-word BIP-39 English phrase.
    #[error("invalid 24-word seed phrase")]
    InvalidSeed,
    /// Wallet bytes did not match the bounded, checksummed Wcash envelope.
    #[error("invalid Wcash wallet bytes: {0}")]
    InvalidWalletBytes(String),
    /// The wallet database could not be read or written.
    #[error("wallet file operation failed: {0}")]
    Io(#[from] io::Error),
    /// Local mobile metadata or checkpointing failed.
    #[error("wallet database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    /// The reviewed Wcash core refused an operation.
    #[error("Wcash core error: {0}")]
    Core(String),
    /// A different send replaced a transaction that was already signed locally.
    #[error("a different Wcash transaction is already signed and awaiting broadcast")]
    SendAlreadyStaged,
    /// Confirm was called without a locally prepared transaction.
    #[error("no Wcash transaction is prepared")]
    NoStagedSend,
    /// JSON passed through the mobile boundary was malformed.
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
enum Runtime {
    Mainnet(WcashMainnetRuntime),
    Testnet(WcashTestnetRuntime),
    #[cfg(feature = "regtest")]
    Regtest(WcashRegtestRuntime),
}

impl Runtime {
    async fn create(
        network: MobileNetwork,
        endpoint: &str,
        wallet_path: &Path,
        seed: &SecretVec<u8>,
    ) -> Result<(Self, InitializedWallet), AdapterError> {
        match network {
            MobileNetwork::Mainnet => WcashMainnetRuntime::create(endpoint, wallet_path, seed)
                .await
                .map(|(runtime, wallet)| (Self::Mainnet(runtime), wallet))
                .map_err(core_error),
            MobileNetwork::Testnet => WcashTestnetRuntime::create(endpoint, wallet_path, seed)
                .await
                .map(|(runtime, wallet)| (Self::Testnet(runtime), wallet))
                .map_err(core_error),
            MobileNetwork::Regtest => {
                #[cfg(feature = "regtest")]
                {
                    WcashRegtestRuntime::create(endpoint, wallet_path, seed)
                        .await
                        .map(|(runtime, wallet)| (Self::Regtest(runtime), wallet))
                        .map_err(core_error)
                }
                #[cfg(not(feature = "regtest"))]
                {
                    let _ = (endpoint, wallet_path, seed);
                    Err(AdapterError::RegtestDisabled)
                }
            }
        }
    }

    async fn restore(
        network: MobileNetwork,
        endpoint: &str,
        wallet_path: &Path,
        seed: &SecretVec<u8>,
        birthday: u32,
    ) -> Result<(Self, InitializedWallet), AdapterError> {
        match network {
            MobileNetwork::Mainnet => {
                WcashMainnetRuntime::restore(endpoint, wallet_path, seed, birthday)
                    .await
                    .map(|(runtime, wallet)| (Self::Mainnet(runtime), wallet))
                    .map_err(core_error)
            }
            MobileNetwork::Testnet => {
                WcashTestnetRuntime::restore(endpoint, wallet_path, seed, birthday)
                    .await
                    .map(|(runtime, wallet)| (Self::Testnet(runtime), wallet))
                    .map_err(core_error)
            }
            MobileNetwork::Regtest => {
                #[cfg(feature = "regtest")]
                {
                    WcashRegtestRuntime::restore(endpoint, wallet_path, seed, birthday)
                        .await
                        .map(|(runtime, wallet)| (Self::Regtest(runtime), wallet))
                        .map_err(core_error)
                }
                #[cfg(not(feature = "regtest"))]
                {
                    let _ = (endpoint, wallet_path, seed, birthday);
                    Err(AdapterError::RegtestDisabled)
                }
            }
        }
    }

    async fn open(
        network: MobileNetwork,
        endpoint: &str,
        wallet_path: &Path,
    ) -> Result<(Self, WalletInfo), AdapterError> {
        match network {
            MobileNetwork::Mainnet => WcashMainnetRuntime::open(endpoint, wallet_path)
                .await
                .map(|(runtime, info)| (Self::Mainnet(runtime), info))
                .map_err(core_error),
            MobileNetwork::Testnet => WcashTestnetRuntime::open(endpoint, wallet_path)
                .await
                .map(|(runtime, info)| (Self::Testnet(runtime), info))
                .map_err(core_error),
            MobileNetwork::Regtest => {
                #[cfg(feature = "regtest")]
                {
                    WcashRegtestRuntime::open(endpoint, wallet_path)
                        .await
                        .map(|(runtime, info)| (Self::Regtest(runtime), info))
                        .map_err(core_error)
                }
                #[cfg(not(feature = "regtest"))]
                {
                    let _ = (endpoint, wallet_path);
                    Err(AdapterError::RegtestDisabled)
                }
            }
        }
    }

    async fn sync(
        &mut self,
        cancellation: &WalletSyncCancellation,
    ) -> Result<WalletBalanceSummary, AdapterError> {
        match self {
            Self::Mainnet(runtime) => runtime.sync(cancellation).await.map_err(core_error),
            Self::Testnet(runtime) => runtime.sync(cancellation).await.map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime.sync(cancellation).await.map_err(core_error),
        }
    }

    fn balance(&self) -> Result<WalletBalanceSummary, AdapterError> {
        match self {
            Self::Mainnet(runtime) => runtime.balance().map_err(core_error),
            Self::Testnet(runtime) => runtime.balance().map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime.balance().map_err(core_error),
        }
    }

    fn receive(&self) -> Result<WalletInfo, AdapterError> {
        match self {
            Self::Mainnet(runtime) => {
                WcashMainnetRuntime::inspect(runtime.wallet_path()).map_err(core_error)
            }
            Self::Testnet(runtime) => {
                WcashTestnetRuntime::inspect(runtime.wallet_path()).map_err(core_error)
            }
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => {
                WcashRegtestRuntime::inspect(runtime.wallet_path()).map_err(core_error)
            }
        }
    }

    fn history(&self, limit: usize) -> Result<ConfirmedTransactionSummaryHistory, AdapterError> {
        match self {
            Self::Mainnet(runtime) => confirmed_transaction_summary_history(
                runtime.wallet_path(),
                WalletNetwork::Mainnet,
                limit,
            )
            .map_err(core_error),
            Self::Testnet(runtime) => confirmed_transaction_summary_history(
                runtime.wallet_path(),
                WalletNetwork::Testnet,
                limit,
            )
            .map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => confirmed_transaction_summary_history(
                runtime.wallet_path(),
                WalletNetwork::Regtest,
                limit,
            )
            .map_err(core_error),
        }
    }
}

fn core_error(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::Core(error.to_string())
}

fn recovery_is_incomplete(error: &AdapterError) -> bool {
    matches!(
        error,
        AdapterError::Core(message)
            if message.contains("transparent recovery is incomplete")
    )
}

/// Seed and birthday shape returned by upstream `init_new` and `init_from_seed`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MobileRecoveryInfo {
    /// The canonical English BIP-39 phrase.
    pub seed_phrase: String,
    /// First height scanned by the wallet.
    pub birthday: u32,
    /// Short chain token used by the existing React Native state.
    pub chain_name: &'static str,
}

/// Stateful first-slice Wcash backend.
pub struct WcashMobileAdapter {
    network: MobileNetwork,
    endpoint: String,
    wallet_path: PathBuf,
    seed_phrase: SecretString,
    birthday: u32,
    runtime: Runtime,
    cancellation: WalletSyncCancellation,
    last_sync: Option<WalletBalanceSummary>,
    pending_send: Option<PendingTransaction>,
}

impl WcashMobileAdapter {
    /// Reads a tip only after the endpoint attests as Wcash Testnet or Regtest.
    ///
    /// The existing mobile contract does not pass a chain hint to this
    /// wallet-less probe, so both available Wcash identities are tried. A
    /// Zcash or otherwise mismatched endpoint fails both attestations.
    pub async fn latest_server_height(endpoint: &str) -> Result<u32, AdapterError> {
        if endpoint.trim().is_empty() {
            return Err(AdapterError::InvalidInput(
                "the endpoint is empty".to_owned(),
            ));
        }
        let mut errors = Vec::new();
        for network in [
            MobileNetwork::Mainnet,
            MobileNetwork::Regtest,
            MobileNetwork::Testnet,
        ] {
            match attested_public_client(endpoint, network.wallet_network()).await {
                Ok(mut client) => {
                    return client
                        .latest_block()
                        .await
                        .map(|block| block.height)
                        .map_err(core_error);
                }
                Err(error) => errors.push(format!("{}: {error}", network.chain_name())),
            }
        }
        Err(AdapterError::Core(format!(
            "endpoint did not attest as a Wcash network ({})",
            errors.join("; ")
        )))
    }

    /// Returns the server metadata shape consumed by the unchanged mobile UI.
    pub async fn server_info_json(
        endpoint: &str,
        network: MobileNetwork,
    ) -> Result<String, AdapterError> {
        let wallet_network = network.wallet_network();
        let mut client = attested_public_client(endpoint, wallet_network)
            .await
            .map_err(core_error)?;
        let latest = client.latest_block().await.map_err(core_error)?;
        let activation_height = network.wallet_activation_height();
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "version": "0.1.0",
            "git_commit": WCASH_WALLET_CORE_REV,
            "server_uri": endpoint,
            "vendor": "Wcash Wallet",
            "taddr_support": true,
            "chain_name": network.chain_name(),
            "sapling_activation_height": activation_height,
            "consensus_branch_id": wallet_network.branch_id_hex(),
            "latest_block_height": latest.height,
            "ironwood_activation_height": activation_height
        }))?)
    }

    /// Returns the immutable Wcash network selected for this wallet.
    pub const fn network(&self) -> MobileNetwork {
        self.network
    }

    /// Returns the endpoint already attested when this runtime was opened.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Generates a 24-word wallet and initializes it against an attested endpoint.
    pub async fn create_new(
        endpoint: &str,
        wallet_path: impl AsRef<Path>,
        network: MobileNetwork,
    ) -> Result<(Self, String), AdapterError> {
        let mnemonic = Mnemonic::<English>::generate(Count::Words24);
        let phrase = mnemonic.phrase().to_owned();
        let seed = SecretVec::new(mnemonic.to_seed("").to_vec());
        let wallet_path = wallet_path.as_ref().to_path_buf();
        let (runtime, initialized) =
            Runtime::create(network, endpoint, &wallet_path, &seed).await?;
        let birthday = initialized.birthday_height;
        let adapter = Self::from_parts(network, endpoint, wallet_path, phrase, birthday, runtime);
        Ok((
            adapter,
            recovery_json(network, birthday, mnemonic.phrase())?,
        ))
    }

    /// Restores a 24-word wallet from an explicit birthday.
    pub async fn restore_from_seed(
        seed_phrase: &str,
        birthday: u32,
        endpoint: &str,
        wallet_path: impl AsRef<Path>,
        network: MobileNetwork,
    ) -> Result<(Self, String), AdapterError> {
        let mnemonic = parse_seed(seed_phrase)?;
        let seed = SecretVec::new(mnemonic.to_seed("").to_vec());
        let wallet_path = wallet_path.as_ref().to_path_buf();
        let (runtime, initialized) =
            Runtime::restore(network, endpoint, &wallet_path, &seed, birthday).await?;
        let birthday = initialized.birthday_height;
        let phrase = mnemonic.phrase().to_owned();
        let adapter = Self::from_parts(network, endpoint, wallet_path, phrase, birthday, runtime);
        Ok((
            adapter,
            recovery_json(network, birthday, mnemonic.phrase())?,
        ))
    }

    /// Opens the checksummed wallet-byte envelope used by the native persistence layer.
    pub async fn open_wallet_bytes(
        bytes: &[u8],
        endpoint: &str,
        wallet_path: impl AsRef<Path>,
        expected_network: MobileNetwork,
    ) -> Result<Self, AdapterError> {
        let decoded = DecodedWallet::decode(bytes)?;
        if decoded.network != expected_network {
            return Err(AdapterError::InvalidWalletBytes(
                "wallet and selected network do not match".to_owned(),
            ));
        }
        let wallet_path = wallet_path.as_ref().to_path_buf();
        let parent = wallet_path.parent().ok_or_else(|| {
            AdapterError::InvalidInput("wallet path has no parent directory".to_owned())
        })?;
        fs::create_dir_all(parent)?;
        set_private_directory_permissions(parent)?;

        // Validate every property in an isolated same-filesystem directory.
        // Nothing below this point may alter an installed wallet until all
        // checks, including seed ownership, have succeeded.
        let validation_directory = tempfile::Builder::new()
            .prefix(".wcash-restore-")
            .tempdir_in(parent)?;
        set_private_directory_permissions(validation_directory.path())?;
        let validation_path = validation_directory.path().join("wallet.sqlite");
        write_private_file(&validation_path, &decoded.database)?;
        let info = inspect_wallet(&validation_path, expected_network.wallet_network())
            .map_err(core_error)?;
        if info.birthday_height != decoded.birthday {
            return Err(AdapterError::InvalidWalletBytes(
                "wallet birthday does not match its database".to_owned(),
            ));
        }
        let mnemonic = parse_seed(&decoded.seed_phrase)?;
        let seed = SecretVec::new(mnemonic.to_seed("").to_vec());
        verify_wallet_seed(&validation_path, expected_network.wallet_network(), &seed).map_err(
            |_| {
                AdapterError::InvalidWalletBytes(
                    "recovery phrase does not control the wallet database".to_owned(),
                )
            },
        )?;
        let (validated_runtime, validated_info) =
            Runtime::open(expected_network, endpoint, &validation_path).await?;
        if validated_info.birthday_height != decoded.birthday {
            return Err(AdapterError::InvalidWalletBytes(
                "attested wallet birthday does not match its database".to_owned(),
            ));
        }
        drop(validated_runtime);

        let mut install = DatabaseInstall::begin(&validation_path, &wallet_path)?;
        let (runtime, installed_info) =
            match Runtime::open(expected_network, endpoint, &wallet_path).await {
                Ok(opened) => opened,
                Err(error) => {
                    install.rollback()?;
                    return Err(error);
                }
            };
        if installed_info.birthday_height != decoded.birthday {
            drop(runtime);
            install.rollback()?;
            return Err(AdapterError::InvalidWalletBytes(
                "installed wallet birthday changed during restore".to_owned(),
            ));
        }
        if verify_wallet_seed(&wallet_path, expected_network.wallet_network(), &seed).is_err() {
            drop(runtime);
            install.rollback()?;
            return Err(AdapterError::InvalidWalletBytes(
                "installed wallet no longer matches its recovery phrase".to_owned(),
            ));
        }
        install.commit();
        Ok(Self::from_parts(
            expected_network,
            endpoint,
            wallet_path,
            decoded.seed_phrase,
            decoded.birthday,
            runtime,
        ))
    }

    fn from_parts(
        network: MobileNetwork,
        endpoint: &str,
        wallet_path: PathBuf,
        seed_phrase: String,
        birthday: u32,
        runtime: Runtime,
    ) -> Self {
        Self {
            network,
            endpoint: endpoint.to_owned(),
            wallet_path,
            seed_phrase: SecretString::new(seed_phrase),
            birthday,
            runtime,
            cancellation: WalletSyncCancellation::new(),
            last_sync: None,
            pending_send: None,
        }
    }

    /// Returns the exact upstream recovery JSON contract.
    pub fn recovery_json(&self) -> Result<String, AdapterError> {
        recovery_json(
            self.network,
            self.birthday,
            self.seed_phrase.expose_secret(),
        )
    }

    /// Returns a fresh cancellation handle before launching a synchronization.
    pub fn prepare_sync(&mut self) -> WalletSyncCancellation {
        self.cancellation = WalletSyncCancellation::new();
        self.cancellation.clone()
    }

    /// Requests cooperative cancellation between the backend's bounded sync batches.
    pub fn pause_sync(&self) -> &'static str {
        self.cancellation.cancel();
        "Pausing sync task..."
    }

    /// Runs one restartable Wcash synchronization and returns upstream poll JSON.
    pub async fn synchronize(&mut self) -> Result<String, AdapterError> {
        if self.cancellation.is_cancelled() {
            self.cancellation = WalletSyncCancellation::new();
        }
        let before = self.runtime.balance().ok();
        let summary = self.runtime.sync(&self.cancellation).await?;
        self.clear_confirmed_pending()?;
        let start = before
            .as_ref()
            .map_or(self.birthday, |value| value.fully_scanned_height);
        let result = SyncComplete {
            sync_start_height: start,
            sync_end_height: summary.fully_scanned_height,
            blocks_scanned: summary.fully_scanned_height.saturating_sub(start),
            sapling_outputs_scanned: 0,
            orchard_outputs_scanned: 0,
            percentage_total_outputs_scanned: if summary.synchronized { 100 } else { 0 },
        };
        self.last_sync = Some(summary);
        Ok(serde_json::to_string_pretty(&SyncPoll {
            sync_complete: result,
        })?)
    }

    /// Returns the available bounded sync status in the existing mobile shape.
    pub fn sync_status_json(&self) -> Result<String, AdapterError> {
        let summary = match self
            .last_sync
            .clone()
            .map(Ok)
            .unwrap_or_else(|| self.runtime.balance())
        {
            Ok(summary) => summary,
            Err(error) if recovery_is_incomplete(&error) => {
                return self.starting_sync_status_json();
            }
            Err(error) => return Err(error),
        };
        let percentage = if summary.synchronized { 100 } else { 0 };
        Ok(serde_json::to_string_pretty(&SyncStatus {
            scan_ranges: synchronized_scan_ranges(
                self.birthday,
                summary.fully_scanned_height,
                summary.synchronized,
            ),
            sync_start_height: summary.fully_scanned_height,
            session_blocks_scanned: 0,
            total_blocks_scanned: summary.fully_scanned_height.saturating_sub(self.birthday),
            percentage_session_blocks_scanned: percentage,
            percentage_total_blocks_scanned: percentage,
            session_ironwood_outputs_scanned: 0,
            total_ironwood_outputs_scanned: 0,
            percentage_session_outputs_scanned: percentage,
            percentage_total_outputs_scanned: percentage,
            total_outputs_scanned: 0,
            total_outputs: 0,
        })?)
    }

    /// Returns the highest fully-scanned wallet height.
    pub fn latest_wallet_block_json(&self) -> Result<String, AdapterError> {
        let height = match self.runtime.balance() {
            Ok(balance) => balance.fully_scanned_height,
            Err(error) if recovery_is_incomplete(&error) => 0,
            Err(error) => return Err(error),
        };
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "height": height
        }))?)
    }

    /// Returns an honest zero-progress status before the first recovery sync.
    pub fn starting_sync_status_json(&self) -> Result<String, AdapterError> {
        Ok(serde_json::to_string_pretty(&SyncStatus {
            scan_ranges: Vec::new(),
            sync_start_height: self.birthday,
            session_blocks_scanned: 0,
            total_blocks_scanned: 0,
            percentage_session_blocks_scanned: 0,
            percentage_total_blocks_scanned: 0,
            session_ironwood_outputs_scanned: 0,
            total_ironwood_outputs_scanned: 0,
            percentage_session_outputs_scanned: 0,
            percentage_total_outputs_scanned: 0,
            total_outputs_scanned: 0,
            total_outputs: 0,
        })?)
    }

    /// Returns pool-separated balances in the existing React Native shape.
    pub fn balance_json(&self) -> Result<String, AdapterError> {
        let balance = match self.runtime.balance() {
            Ok(summary) => mobile_balance(&summary),
            Err(error) if recovery_is_incomplete(&error) => MobileBalance::default(),
            Err(error) => return Err(error),
        };
        Ok(serde_json::to_string_pretty(&balance)?)
    }

    /// Returns the spendable Ironwood total in the existing mobile shape.
    pub fn spendable_balance_json(&self) -> Result<String, AdapterError> {
        let spendable_balance = match self.runtime.balance() {
            Ok(balance) => balance.accounts.iter().fold(0_u64, |total, account| {
                total.saturating_add(account.ironwood_spendable_zat)
            }),
            Err(error) if recovery_is_incomplete(&error) => 0,
            Err(error) => return Err(error),
        };
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "spendable_balance": spendable_balance
        }))?)
    }

    /// Returns the account's canonical Wcash Unified Address in the upstream list shape.
    pub fn unified_addresses_json(&self) -> Result<String, AdapterError> {
        let info = self.runtime.receive()?;
        Ok(serde_json::to_string_pretty(&vec![MobileUnifiedAddress {
            account: 0,
            address_index: 0,
            has_orchard: true,
            has_sapling: false,
            has_transparent: false,
            encoded_address: info.address,
        }])?)
    }

    /// Returns the transparent coinbase receiver in the upstream list shape.
    pub fn transparent_addresses_json(&self) -> Result<String, AdapterError> {
        let info = self.runtime.receive()?;
        Ok(serde_json::to_string_pretty(&vec![
            MobileTransparentAddress {
                account: 0,
                address_index: 0,
                scope: "external",
                encoded_address: info.transparent_coinbase_address,
            },
        ])?)
    }

    /// Validates a Wcash Ironwood recipient and returns upstream parse-address JSON.
    pub fn parse_address_json(address: &str) -> Result<String, AdapterError> {
        if address.trim().is_empty() {
            return Err(AdapterError::InvalidInput(
                "the address is empty".to_owned(),
            ));
        }
        let network = [
            MobileNetwork::Mainnet,
            MobileNetwork::Testnet,
            MobileNetwork::Regtest,
        ]
        .into_iter()
        .find(|network| decode_recipient(address, network.wallet_network()).is_ok())
        .ok_or_else(|| AdapterError::InvalidInput("invalid Wcash address".to_owned()))?;
        Ok(serde_json::to_string_pretty(&ParsedAddress {
            status: "success",
            chain_name: network.chain_name(),
            address_kind: "unified",
            receivers_available: vec!["orchard"],
            shielded_only_ua: address,
        })?)
    }

    /// Validates a recipient against this wallet's immutable Wcash network.
    pub fn parse_recipient_json(&self, address: &str) -> Result<String, AdapterError> {
        parse_address_for_network_json(address, self.network)
    }

    /// Returns bounded, newest-first confirmed and locally-pending history in
    /// the upstream list shape.
    pub fn confirmed_history_json(&self, limit: usize) -> Result<String, AdapterError> {
        if limit == 0 || limit > MAX_HISTORY_ROWS {
            return Err(AdapterError::InvalidInput(format!(
                "history limit must be between 1 and {MAX_HISTORY_ROWS}"
            )));
        }
        let mut history = match self.runtime.history(limit) {
            Ok(history) => mobile_history(history),
            Err(error) if recovery_is_incomplete(&error) => MobileHistory {
                value_transfers: Vec::new(),
                total: 0,
            },
            Err(error) => return Err(error),
        };
        self.apply_stored_metadata_to_confirmed(&mut history.value_transfers)?;
        let mut pending = self.pending_history()?;
        pending.extend(history.value_transfers);
        pending.truncate(limit);
        history.value_transfers = pending;
        history.total = history.value_transfers.len();
        Ok(serde_json::to_string_pretty(&history)?)
    }

    /// Selects and locks one exact Wcash proposal without loading spending
    /// authority, returning its exact ZIP 317 fee before user consent.
    pub async fn stage_send_json(&mut self, send_json: &str) -> Result<String, AdapterError> {
        let requests: Vec<MobilePayment> = serde_json::from_str(send_json)?;
        if requests.is_empty() {
            return Err(AdapterError::InvalidInput(
                "no recipients supplied".to_owned(),
            ));
        }
        let mut payments = Vec::with_capacity(requests.len());
        for request in requests {
            if request.amount == 0 {
                return Err(AdapterError::InvalidInput(
                    "payment amount must be greater than zero".to_owned(),
                ));
            }
            decode_recipient(&request.address, self.network.wallet_network())
                .map_err(|error| AdapterError::InvalidInput(error.to_string()))?;
            let memo = request.memo.unwrap_or_default().into_bytes();
            if memo.len() > 512 {
                return Err(AdapterError::InvalidInput(
                    "memo exceeds 512 bytes".to_owned(),
                ));
            }
            payments.push(TransferRecipient {
                address: request.address,
                amount_zat: request.amount,
                memo,
            });
        }
        let request = PendingRequest::Send(payments.clone());
        if self
            .pending_send
            .as_ref()
            .is_some_and(|pending| pending.request == request)
        {
            return self.pending_proposal_json();
        }
        self.cancel_replaceable_preview()?;
        let staged = propose_transfer_offline(
            &self.wallet_path,
            self.network.wallet_network(),
            payments,
            self.minimum_confirmations(),
            self.network == MobileNetwork::Regtest,
            TRANSACTION_EXPIRY_DELTA,
            TRANSACTION_LOCK_BLOCKS,
        )
        .map_err(core_error)?;
        let fee = staged.fee_zat();
        self.pending_send = Some(PendingTransaction {
            request,
            fee_zat: fee,
            value_to_shield_zat: None,
            staged: Some(staged),
            calculated: None,
        });
        let response = SendProposal {
            fee,
            source_pools: vec!["ironwood"],
            destination_pools: vec!["ironwood"],
        };
        Ok(serde_json::to_string_pretty(&response)?)
    }

    /// Previews mature transparent coinbase shielding with an exact fee and
    /// value while retaining the same confirmation flow as upstream.
    pub fn stage_shield_json(&mut self) -> Result<String, AdapterError> {
        let request = PendingRequest::Shield;
        if self
            .pending_send
            .as_ref()
            .is_some_and(|pending| pending.request == request)
        {
            return self.pending_proposal_json();
        }
        self.cancel_replaceable_preview()?;
        let staged = propose_coinbase_shielding_offline(
            &self.wallet_path,
            self.network.wallet_network(),
            MAX_COINBASE_SHIELDING_INPUTS,
            TRANSACTION_EXPIRY_DELTA,
            TRANSACTION_LOCK_BLOCKS,
        )
        .map_err(core_error)?;
        let fee_zat = staged.fee_zat();
        let value_to_shield_zat = staged.value_to_shield_zat().ok_or_else(|| {
            AdapterError::Core("shield proposal omitted its exact value".to_owned())
        })?;
        self.pending_send = Some(PendingTransaction {
            request,
            fee_zat,
            value_to_shield_zat: Some(value_to_shield_zat),
            staged: Some(staged),
            calculated: None,
        });
        self.pending_proposal_json()
    }

    /// Verifies seed ownership again, signs the exact consented proposal,
    /// persists its pending projection, and broadcasts those same bytes.
    pub async fn confirm_send_json(&mut self) -> Result<String, AdapterError> {
        let mut pending = self.pending_send.take().ok_or(AdapterError::NoStagedSend)?;
        let mnemonic = match parse_seed(self.seed_phrase.expose_secret()) {
            Ok(mnemonic) => mnemonic,
            Err(error) => {
                self.pending_send = Some(pending);
                return Err(error);
            }
        };
        let seed = SecretVec::new(mnemonic.to_seed("").to_vec());
        if let Err(error) =
            verify_wallet_seed(&self.wallet_path, self.network.wallet_network(), &seed)
        {
            self.pending_send = Some(pending);
            return Err(core_error(error));
        }
        if pending.calculated.is_none() {
            let Some(staged) = pending.staged.as_ref() else {
                self.pending_send = Some(pending);
                return Err(AdapterError::NoStagedSend);
            };
            match calculate_staged_transaction(
                &self.wallet_path,
                self.network.wallet_network(),
                &seed,
                staged,
            ) {
                Ok(calculated) => pending.calculated = Some(calculated),
                Err(error) => {
                    self.pending_send = Some(pending);
                    return Err(core_error(error));
                }
            }
        }
        let Some(calculated) = pending.calculated.as_ref() else {
            self.pending_send = Some(pending);
            return Err(AdapterError::NoStagedSend);
        };
        if calculated.signed().fee_zat != pending.fee_zat {
            self.pending_send = Some(pending);
            return Err(AdapterError::Core(
                "calculated fee differs from the consented proposal".to_owned(),
            ));
        }
        if let Err(error) = self.persist_pending_metadata(&pending, "calculated") {
            self.pending_send = Some(pending);
            return Err(error);
        }
        if let Err(error) = self.checkpoint_wallet_database() {
            self.pending_send = Some(pending);
            return Err(error);
        }
        let mut client =
            match AttestedWcashClient::connect(&self.endpoint, self.network.wallet_network()).await
            {
                Ok(client) => client,
                Err(error) => {
                    self.pending_send = Some(pending);
                    return Err(core_error(error));
                }
            };
        let txid = match broadcast_calculated_transaction(&mut client, calculated).await {
            Ok(result) => result.txid,
            Err(error) => {
                self.pending_send = Some(pending);
                return Err(core_error(error));
            }
        };
        // The node accepted these exact bytes. A local follow-up write cannot
        // turn that fact into a send error that would invite a replacement.
        // The pre-broadcast `calculated` row and signed bytes are already
        // durable and remain visible after a crash.
        let persistence = self
            .update_pending_state(&txid, "transmitted")
            .and_then(|()| self.checkpoint_wallet_database());
        accepted_broadcast_result(&mut self.pending_send, pending, txid, persistence)
    }

    /// Discards an unsigned preview and releases only its proposal-owned locks.
    pub fn discard_staged_send(&mut self) -> Result<(), AdapterError> {
        self.cancel_replaceable_preview()
    }

    fn minimum_confirmations(&self) -> u32 {
        match self.network {
            MobileNetwork::Mainnet => COINBASE_SHIELDING_MATURITY,
            MobileNetwork::Testnet => COINBASE_SHIELDING_MATURITY,
            MobileNetwork::Regtest => LOCAL_REGTEST_CONFIRMATIONS,
        }
    }

    fn pending_proposal_json(&self) -> Result<String, AdapterError> {
        let pending = self
            .pending_send
            .as_ref()
            .ok_or(AdapterError::NoStagedSend)?;
        match pending.request {
            PendingRequest::Send(_) => Ok(serde_json::to_string_pretty(&SendProposal {
                fee: pending.fee_zat,
                source_pools: vec!["ironwood"],
                destination_pools: vec!["ironwood"],
            })?),
            PendingRequest::Shield => Ok(serde_json::to_string_pretty(&serde_json::json!({
                "fee": pending.fee_zat,
                "value_to_shield": pending.value_to_shield_zat.ok_or_else(|| {
                    AdapterError::Core("shield preview is missing its exact value".to_owned())
                })?
            }))?),
        }
    }

    fn cancel_replaceable_preview(&mut self) -> Result<(), AdapterError> {
        let Some(pending) = self.pending_send.take() else {
            return Ok(());
        };
        if pending.calculated.is_some() {
            self.pending_send = Some(pending);
            return Err(AdapterError::SendAlreadyStaged);
        }
        if let Some(staged) = pending.staged.as_ref()
            && let Err(error) =
                cancel_staged_transaction(&self.wallet_path, self.network.wallet_network(), staged)
        {
            self.pending_send = Some(pending);
            return Err(core_error(error));
        }
        Ok(())
    }

    fn ensure_pending_table(connection: &Connection) -> Result<(), AdapterError> {
        connection.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {MOBILE_PENDING_TABLE} (
                txid TEXT PRIMARY KEY NOT NULL,
                kind TEXT NOT NULL CHECK (kind IN ('sent', 'shield')),
                value_zat INTEGER NOT NULL CHECK (value_zat >= 0),
                fee_zat INTEGER NOT NULL CHECK (fee_zat >= 0),
                state TEXT NOT NULL CHECK (state IN ('calculated', 'transmitted')),
                recipient_address TEXT,
                created_at INTEGER NOT NULL
            );"
        ))?;
        Ok(())
    }

    fn persist_pending_metadata(
        &self,
        pending: &PendingTransaction,
        state: &'static str,
    ) -> Result<(), AdapterError> {
        let signed = pending
            .calculated
            .as_ref()
            .ok_or(AdapterError::NoStagedSend)?
            .signed();
        let (kind, value_zat, recipient_address) = match &pending.request {
            PendingRequest::Send(payments) => (
                "sent",
                payments.iter().try_fold(0_u64, |total, payment| {
                    total.checked_add(payment.amount_zat).ok_or_else(|| {
                        AdapterError::InvalidInput("payment total overflows u64".to_owned())
                    })
                })?,
                payments.first().map(|payment| payment.address.as_str()),
            ),
            PendingRequest::Shield => (
                "shield",
                pending.value_to_shield_zat.ok_or_else(|| {
                    AdapterError::Core("shield proposal omitted its exact value".to_owned())
                })?,
                None,
            ),
        };
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| AdapterError::Core(error.to_string()))?
            .as_secs();
        let connection = Connection::open(&self.wallet_path)?;
        Self::ensure_pending_table(&connection)?;
        connection.execute(
            &format!(
                "INSERT INTO {MOBILE_PENDING_TABLE}
                    (txid, kind, value_zat, fee_zat, state, recipient_address, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(txid) DO UPDATE SET state = excluded.state"
            ),
            params![
                signed.txid,
                kind,
                i64::try_from(value_zat).map_err(|_| AdapterError::InvalidInput(
                    "pending value is too large".to_owned()
                ))?,
                i64::try_from(pending.fee_zat).map_err(|_| AdapterError::InvalidInput(
                    "pending fee is too large".to_owned()
                ))?,
                state,
                recipient_address,
                i64::try_from(created_at).unwrap_or(i64::MAX),
            ],
        )?;
        Ok(())
    }

    fn update_pending_state(&self, txid: &str, state: &'static str) -> Result<(), AdapterError> {
        let connection = Connection::open(&self.wallet_path)?;
        Self::ensure_pending_table(&connection)?;
        let changed = connection.execute(
            &format!("UPDATE {MOBILE_PENDING_TABLE} SET state = ?1 WHERE txid = ?2"),
            params![state, txid],
        )?;
        if changed != 1 {
            return Err(AdapterError::Core(
                "pending transaction metadata disappeared before save".to_owned(),
            ));
        }
        Ok(())
    }

    fn checkpoint_wallet_database(&self) -> Result<(), AdapterError> {
        let connection = Connection::open(&self.wallet_path)?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    fn clear_confirmed_pending(&mut self) -> Result<(), AdapterError> {
        let Some(txid) = self
            .pending_send
            .as_ref()
            .and_then(|pending| pending.calculated.as_ref())
            .map(|calculated| calculated.signed().txid.clone())
        else {
            return Ok(());
        };
        let confirmed = self.runtime.history(MAX_HISTORY_ROWS)?;
        if confirmed
            .transactions
            .iter()
            .any(|transaction| transaction.transaction.txid == txid)
        {
            self.pending_send = None;
        }
        Ok(())
    }

    fn pending_history(&self) -> Result<Vec<MobileValueTransfer>, AdapterError> {
        let page = match active_pending_signed_transactions(
            &self.wallet_path,
            self.network.wallet_network(),
            None,
            None,
            MAX_PENDING_TRANSACTION_PAGE_SIZE,
        ) {
            Ok(page) => page,
            Err(error) if error.to_string().contains("recovery is incomplete") => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(core_error(error)),
        };
        if page.transactions.is_empty() {
            return Ok(Vec::new());
        }
        let mut active = page
            .transactions
            .into_iter()
            .map(|transaction| transaction.txid)
            .collect::<std::collections::HashSet<_>>();
        let mut result = Vec::new();
        for row in self.stored_pending_metadata()? {
            if !active.contains(&row.txid) {
                continue;
            }
            active.remove(&row.txid);
            result.push(row.into_mobile_value_transfer()?);
        }
        for txid in active {
            result.push(MobileValueTransfer {
                txid,
                datetime: 0,
                status: "calculated",
                blockheight: 0,
                transaction_fee: None,
                kind: "unknown",
                // Upstream's required numeric field uses zero only as the
                // explicit unavailable sentinel; kind remains unknown.
                value: 0,
                pools_sent_from: Vec::new(),
                pools_received: Vec::new(),
                recipient_address: None,
            });
        }
        Ok(result)
    }

    fn stored_pending_metadata(&self) -> Result<Vec<PendingMetadata>, AdapterError> {
        let connection = Connection::open(&self.wallet_path)?;
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [MOBILE_PENDING_TABLE],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut statement = connection.prepare(&format!(
            "SELECT txid, kind, value_zat, fee_zat, state, recipient_address, created_at
             FROM {MOBILE_PENDING_TABLE} ORDER BY created_at DESC"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(PendingMetadata {
                txid: row.get(0)?,
                kind: row.get(1)?,
                value_zat: row.get::<_, i64>(2)?,
                fee_zat: row.get::<_, i64>(3)?,
                state: row.get(4)?,
                recipient_address: row.get(5)?,
                created_at: row.get::<_, i64>(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn apply_stored_metadata_to_confirmed(
        &self,
        confirmed: &mut [MobileValueTransfer],
    ) -> Result<(), AdapterError> {
        let metadata = self.stored_pending_metadata()?;
        for row in confirmed {
            if let Some(stored) = metadata.iter().find(|stored| stored.txid == row.txid) {
                stored.apply_to_confirmed(row)?;
            }
        }
        Ok(())
    }

    /// Serializes the seed plus SQLite database into bounded checksummed wallet bytes.
    pub fn save_wallet_bytes(&self) -> Result<Vec<u8>, AdapterError> {
        self.checkpoint_wallet_database()?;
        let database = fs::read(&self.wallet_path)?;
        encode_wallet(
            self.network,
            self.birthday,
            self.seed_phrase.expose_secret(),
            &database,
        )
    }

    /// Explicitly fails features whose Wcash core contracts do not yet exist.
    pub fn unsupported(feature: &'static str) -> AdapterError {
        AdapterError::UnsupportedFeature(feature)
    }
}

fn parse_address_for_network_json(
    address: &str,
    network: MobileNetwork,
) -> Result<String, AdapterError> {
    if address.trim().is_empty() {
        return Err(AdapterError::InvalidInput(
            "the address is empty".to_owned(),
        ));
    }
    decode_recipient(address, network.wallet_network())
        .map_err(|error| AdapterError::InvalidInput(error.to_string()))?;
    Ok(serde_json::to_string_pretty(&ParsedAddress {
        status: "success",
        chain_name: network.chain_name(),
        address_kind: "unified",
        receivers_available: vec!["orchard"],
        shielded_only_ua: address,
    })?)
}

fn accepted_broadcast_result(
    slot: &mut Option<PendingTransaction>,
    pending: PendingTransaction,
    txid: String,
    _persistence: Result<(), AdapterError>,
) -> Result<String, AdapterError> {
    *slot = Some(pending);
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "txids": [txid]
    }))?)
}

fn parse_seed(seed_phrase: &str) -> Result<Mnemonic<English>, AdapterError> {
    let mnemonic =
        Mnemonic::<English>::from_phrase(seed_phrase).map_err(|_| AdapterError::InvalidSeed)?;
    if mnemonic.phrase().split_whitespace().count() != 24 {
        return Err(AdapterError::InvalidSeed);
    }
    Ok(mnemonic)
}

fn recovery_json(
    network: MobileNetwork,
    birthday: u32,
    seed_phrase: &str,
) -> Result<String, AdapterError> {
    Ok(serde_json::to_string_pretty(&MobileRecoveryInfo {
        seed_phrase: seed_phrase.to_owned(),
        birthday,
        chain_name: network.chain_name(),
    })?)
}

#[derive(Default, Serialize)]
struct MobileBalance {
    total_orchard_balance: u64,
    total_sapling_balance: u64,
    total_transparent_balance: u64,
    confirmed_transparent_balance: u64,
    confirmed_orchard_balance: u64,
    confirmed_sapling_balance: u64,
    unconfirmed_orchard_balance: u64,
    unconfirmed_sapling_balance: u64,
    unconfirmed_transparent_balance: u64,
    total_ironwood_balance: u64,
    confirmed_ironwood_balance: u64,
    unconfirmed_ironwood_balance: u64,
}

fn mobile_balance(summary: &WalletBalanceSummary) -> MobileBalance {
    let mut result = MobileBalance {
        total_orchard_balance: 0,
        total_sapling_balance: 0,
        total_transparent_balance: 0,
        confirmed_transparent_balance: 0,
        confirmed_orchard_balance: 0,
        confirmed_sapling_balance: 0,
        unconfirmed_orchard_balance: 0,
        unconfirmed_sapling_balance: 0,
        unconfirmed_transparent_balance: 0,
        total_ironwood_balance: 0,
        confirmed_ironwood_balance: 0,
        unconfirmed_ironwood_balance: 0,
    };
    for account in &summary.accounts {
        result.total_ironwood_balance = result
            .total_ironwood_balance
            .saturating_add(account.ironwood_total_zat);
        result.confirmed_ironwood_balance = result.confirmed_ironwood_balance.saturating_add(
            account
                .ironwood_total_zat
                .saturating_sub(account.ironwood_pending_change_zat)
                .saturating_sub(account.ironwood_pending_spendability_zat),
        );
        result.total_orchard_balance = result
            .total_orchard_balance
            .saturating_add(account.orchard_total_zat);
        result.total_sapling_balance = result
            .total_sapling_balance
            .saturating_add(account.sapling_total_zat);
        result.total_transparent_balance = result
            .total_transparent_balance
            .saturating_add(account.transparent_total_zat);
        result.confirmed_transparent_balance = result.confirmed_transparent_balance.saturating_add(
            account
                .transparent_total_zat
                .saturating_sub(account.transparent_coinbase_pending_zat),
        );
    }
    result.unconfirmed_ironwood_balance = result
        .total_ironwood_balance
        .saturating_sub(result.confirmed_ironwood_balance);
    result.confirmed_orchard_balance = result.total_orchard_balance;
    result.confirmed_sapling_balance = result.total_sapling_balance;
    result.unconfirmed_transparent_balance = result
        .total_transparent_balance
        .saturating_sub(result.confirmed_transparent_balance);
    result
}

#[derive(Serialize)]
struct MobileUnifiedAddress {
    account: u32,
    address_index: u32,
    has_orchard: bool,
    has_sapling: bool,
    has_transparent: bool,
    encoded_address: String,
}

#[derive(Serialize)]
struct MobileTransparentAddress {
    account: u32,
    address_index: u32,
    scope: &'static str,
    encoded_address: String,
}

#[derive(Serialize)]
struct ParsedAddress<'a> {
    status: &'static str,
    chain_name: &'static str,
    address_kind: &'static str,
    receivers_available: Vec<&'static str>,
    shielded_only_ua: &'a str,
}

#[derive(Serialize)]
struct MobileHistory {
    value_transfers: Vec<MobileValueTransfer>,
    total: usize,
}

#[derive(Serialize)]
struct MobileValueTransfer {
    txid: String,
    datetime: u32,
    status: &'static str,
    blockheight: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction_fee: Option<u64>,
    kind: &'static str,
    value: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pools_sent_from: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pools_received: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recipient_address: Option<String>,
}

fn mobile_history(history: ConfirmedTransactionSummaryHistory) -> MobileHistory {
    let value_transfers = history
        .transactions
        .into_iter()
        .map(mobile_value_transfer)
        .collect::<Vec<_>>();
    let total = value_transfers.len();
    MobileHistory {
        value_transfers,
        total,
    }
}

fn mobile_value_transfer(summary: ConfirmedTransactionSummary) -> MobileValueTransfer {
    let tx = summary.transaction;
    let Some(value) = summary.value_zat else {
        return MobileValueTransfer {
            txid: tx.txid,
            datetime: tx.timestamp.unwrap_or(0),
            status: "confirmed",
            blockheight: tx.mined_height,
            transaction_fee: None,
            kind: "unknown",
            // Preserve the upstream JSON schema without guessing a transfer
            // amount. The unknown kind makes zero an availability sentinel.
            value: 0,
            pools_sent_from: Vec::new(),
            pools_received: Vec::new(),
            recipient_address: None,
        };
    };
    let kind = match (tx.direction, tx.kind) {
        (ConfirmedTransactionDirection::Incoming, _) => "received",
        (ConfirmedTransactionDirection::Outgoing, _) => "sent",
        (ConfirmedTransactionDirection::Internal, ConfirmedTransactionKind::Shielding) => "shield",
        (ConfirmedTransactionDirection::Internal, ConfirmedTransactionKind::Migration) => {
            "migration"
        }
        (ConfirmedTransactionDirection::Internal, _) => "send-to-self",
    };
    let pools_sent_from = match (tx.direction, tx.kind) {
        (ConfirmedTransactionDirection::Incoming, _) => Vec::new(),
        (_, ConfirmedTransactionKind::Shielding) => vec!["Transparent"],
        (_, ConfirmedTransactionKind::Migration) => vec!["Orchard"],
        _ => vec!["Ironwood"],
    };
    let pools_received = match (tx.direction, tx.kind) {
        (ConfirmedTransactionDirection::Outgoing, _) => Vec::new(),
        (_, ConfirmedTransactionKind::Coinbase) => vec!["Transparent"],
        _ => vec!["Ironwood"],
    };
    MobileValueTransfer {
        txid: tx.txid,
        datetime: tx.timestamp.unwrap_or(0),
        status: "confirmed",
        blockheight: tx.mined_height,
        transaction_fee: tx.fee_zat,
        kind,
        value,
        pools_sent_from,
        pools_received,
        recipient_address: None,
    }
}

#[derive(Deserialize)]
struct MobilePayment {
    address: String,
    amount: u64,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Eq, PartialEq)]
enum PendingRequest {
    Send(Vec<TransferRecipient>),
    Shield,
}

struct PendingTransaction {
    request: PendingRequest,
    fee_zat: u64,
    value_to_shield_zat: Option<u64>,
    staged: Option<StagedTransactionProposal>,
    calculated: Option<CalculatedTransaction>,
}

struct PendingMetadata {
    txid: String,
    kind: String,
    value_zat: i64,
    fee_zat: i64,
    state: String,
    recipient_address: Option<String>,
    created_at: i64,
}

impl PendingMetadata {
    fn apply_to_confirmed(&self, row: &mut MobileValueTransfer) -> Result<(), AdapterError> {
        if !matches!(self.state.as_str(), "calculated" | "transmitted") {
            return Err(AdapterError::Core(
                "pending transaction state is invalid".to_owned(),
            ));
        }
        let value = u64::try_from(self.value_zat)
            .map_err(|_| AdapterError::Core("pending transaction value is negative".to_owned()))?;
        let fee = u64::try_from(self.fee_zat)
            .map_err(|_| AdapterError::Core("pending transaction fee is negative".to_owned()))?;
        row.value = value;
        row.transaction_fee = Some(fee);
        row.recipient_address.clone_from(&self.recipient_address);
        match self.kind.as_str() {
            "sent" => {
                row.kind = "sent";
                row.pools_sent_from = vec!["Ironwood"];
                row.pools_received.clear();
            }
            "shield" => {
                row.kind = "shield";
                row.pools_sent_from = vec!["Transparent"];
                row.pools_received = vec!["Ironwood"];
            }
            _ => {
                return Err(AdapterError::Core(
                    "pending transaction kind is invalid".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn into_mobile_value_transfer(self) -> Result<MobileValueTransfer, AdapterError> {
        let shielding = match self.kind.as_str() {
            "sent" => false,
            "shield" => true,
            _ => {
                return Err(AdapterError::Core(
                    "pending transaction kind is invalid".to_owned(),
                ));
            }
        };
        let status = match self.state.as_str() {
            "calculated" => "calculated",
            "transmitted" => "transmitted",
            _ => {
                return Err(AdapterError::Core(
                    "pending transaction state is invalid".to_owned(),
                ));
            }
        };
        let value = u64::try_from(self.value_zat)
            .map_err(|_| AdapterError::Core("pending transaction value is negative".to_owned()))?;
        let fee = u64::try_from(self.fee_zat)
            .map_err(|_| AdapterError::Core("pending transaction fee is negative".to_owned()))?;
        Ok(MobileValueTransfer {
            txid: self.txid,
            datetime: u32::try_from(self.created_at).unwrap_or(u32::MAX),
            status,
            blockheight: 0,
            transaction_fee: Some(fee),
            kind: if shielding { "shield" } else { "sent" },
            value,
            pools_sent_from: if shielding {
                vec!["Transparent"]
            } else {
                vec!["Ironwood"]
            },
            pools_received: if shielding {
                vec!["Ironwood"]
            } else {
                Vec::new()
            },
            recipient_address: self.recipient_address,
        })
    }
}

#[derive(Serialize)]
struct SendProposal {
    fee: u64,
    source_pools: Vec<&'static str>,
    destination_pools: Vec<&'static str>,
}

#[derive(Serialize)]
struct SyncPoll {
    sync_complete: SyncComplete,
}

#[derive(Serialize)]
struct SyncComplete {
    sync_start_height: u32,
    sync_end_height: u32,
    blocks_scanned: u32,
    sapling_outputs_scanned: u64,
    orchard_outputs_scanned: u64,
    percentage_total_outputs_scanned: u32,
}

#[derive(Serialize)]
struct SyncStatus {
    scan_ranges: Vec<SyncScanRange>,
    sync_start_height: u32,
    session_blocks_scanned: u32,
    total_blocks_scanned: u32,
    percentage_session_blocks_scanned: u32,
    percentage_total_blocks_scanned: u32,
    session_ironwood_outputs_scanned: u64,
    total_ironwood_outputs_scanned: u64,
    percentage_session_outputs_scanned: u32,
    percentage_total_outputs_scanned: u32,
    total_outputs_scanned: u64,
    total_outputs: u64,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
struct SyncScanRange {
    priority: &'static str,
    start_block: u32,
    end_block: u32,
}

fn synchronized_scan_ranges(
    birthday: u32,
    fully_scanned_height: u32,
    synchronized: bool,
) -> Vec<SyncScanRange> {
    if !synchronized || fully_scanned_height < birthday {
        return Vec::new();
    }
    vec![SyncScanRange {
        priority: "Scanned",
        start_block: birthday,
        end_block: fully_scanned_height,
    }]
}

struct DecodedWallet {
    network: MobileNetwork,
    birthday: u32,
    seed_phrase: String,
    database: Vec<u8>,
}

impl DecodedWallet {
    fn decode(bytes: &[u8]) -> Result<Self, AdapterError> {
        if bytes.len() < WALLET_FILE_HEADER_LEN {
            return Err(AdapterError::InvalidWalletBytes(
                "file is truncated".to_owned(),
            ));
        }
        let version = u64::from_le_bytes(bytes[0..8].try_into().expect("fixed slice"));
        if version != WALLET_FILE_VERSION || &bytes[8..16] != WALLET_FILE_MAGIC {
            return Err(AdapterError::InvalidWalletBytes(
                "wrong version or application magic".to_owned(),
            ));
        }
        let network = MobileNetwork::from_envelope_byte(bytes[16])?;
        let birthday = u32::from_le_bytes(bytes[17..21].try_into().expect("fixed slice"));
        let phrase_len =
            u32::from_le_bytes(bytes[21..25].try_into().expect("fixed slice")) as usize;
        let database_len =
            u64::from_le_bytes(bytes[25..33].try_into().expect("fixed slice")) as usize;
        if phrase_len == 0 || phrase_len > MAX_SEED_PHRASE_BYTES {
            return Err(AdapterError::InvalidWalletBytes(
                "seed phrase length is outside its bound".to_owned(),
            ));
        }
        if database_len == 0 || database_len > MAX_WALLET_DATABASE_BYTES {
            return Err(AdapterError::InvalidWalletBytes(
                "database length is outside its bound".to_owned(),
            ));
        }
        let expected_len = WALLET_FILE_HEADER_LEN
            .checked_add(phrase_len)
            .and_then(|value| value.checked_add(database_len))
            .ok_or_else(|| AdapterError::InvalidWalletBytes("length overflow".to_owned()))?;
        if bytes.len() != expected_len {
            return Err(AdapterError::InvalidWalletBytes(
                "file length does not match its header".to_owned(),
            ));
        }
        let payload = &bytes[WALLET_FILE_HEADER_LEN..];
        let actual_digest = Sha256::digest(payload);
        if actual_digest.as_slice() != &bytes[33..65] {
            return Err(AdapterError::InvalidWalletBytes(
                "checksum mismatch".to_owned(),
            ));
        }
        let seed_phrase = String::from_utf8(payload[..phrase_len].to_vec())
            .map_err(|_| AdapterError::InvalidWalletBytes("seed phrase is not UTF-8".to_owned()))?;
        parse_seed(&seed_phrase)
            .map_err(|_| AdapterError::InvalidWalletBytes("seed phrase is invalid".to_owned()))?;
        Ok(Self {
            network,
            birthday,
            seed_phrase,
            database: payload[phrase_len..].to_vec(),
        })
    }
}

fn encode_wallet(
    network: MobileNetwork,
    birthday: u32,
    seed_phrase: &str,
    database: &[u8],
) -> Result<Vec<u8>, AdapterError> {
    parse_seed(seed_phrase)?;
    if seed_phrase.len() > MAX_SEED_PHRASE_BYTES {
        return Err(AdapterError::InvalidWalletBytes(
            "seed phrase length is outside its bound".to_owned(),
        ));
    }
    if database.is_empty() || database.len() > MAX_WALLET_DATABASE_BYTES {
        return Err(AdapterError::InvalidWalletBytes(
            "database length is outside its bound".to_owned(),
        ));
    }
    let mut payload = Vec::with_capacity(seed_phrase.len() + database.len());
    payload.extend_from_slice(seed_phrase.as_bytes());
    payload.extend_from_slice(database);
    let digest = Sha256::digest(&payload);
    let mut bytes = Vec::with_capacity(WALLET_FILE_HEADER_LEN + payload.len());
    bytes.extend_from_slice(&WALLET_FILE_VERSION.to_le_bytes());
    bytes.extend_from_slice(WALLET_FILE_MAGIC);
    bytes.push(network.envelope_byte());
    bytes.extend_from_slice(&birthday.to_le_bytes());
    bytes.extend_from_slice(&(seed_phrase.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(database.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&digest);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn set_private_directory_permissions(path: &Path) -> Result<(), AdapterError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private_file(path: &Path, database: &[u8]) -> Result<(), AdapterError> {
    fs::write(path, database)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

struct DatabaseInstall {
    parts: Vec<DatabasePart>,
    // Kept as a normal private directory so an OS-level rollback failure can
    // never cause TempDir::drop to delete the only copy of the old wallet.
    rollback_directory: PathBuf,
    committed: bool,
}

struct DatabasePart {
    original: PathBuf,
    backup: Option<PathBuf>,
    restored: bool,
}

impl DatabaseInstall {
    fn begin(validated: &Path, destination: &Path) -> Result<Self, AdapterError> {
        let parent = destination.parent().ok_or_else(|| {
            AdapterError::InvalidInput("wallet path has no parent directory".to_owned())
        })?;
        let rollback_directory = tempfile::Builder::new()
            .prefix(".wcash-rollback-")
            .tempdir_in(parent)?;
        set_private_directory_permissions(rollback_directory.path())?;
        let rollback_directory = rollback_directory.keep();
        let destinations = [
            destination.to_path_buf(),
            PathBuf::from(format!("{}-wal", destination.display())),
            PathBuf::from(format!("{}-shm", destination.display())),
        ];
        let mut install = Self {
            parts: Vec::with_capacity(destinations.len()),
            rollback_directory,
            committed: false,
        };
        for (index, original) in destinations.into_iter().enumerate() {
            let backup = if original.exists() {
                let backup = install.rollback_directory.join(format!("original-{index}"));
                if let Err(error) = fs::rename(&original, &backup) {
                    let _ = install.rollback();
                    return Err(error.into());
                }
                Some(backup)
            } else {
                None
            };
            install.parts.push(DatabasePart {
                original,
                backup,
                restored: false,
            });
        }
        if let Err(error) = fs::rename(validated, destination) {
            let _ = install.rollback();
            return Err(error.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(error) = fs::set_permissions(destination, fs::Permissions::from_mode(0o600))
            {
                let _ = install.rollback();
                return Err(error.into());
            }
        }
        Ok(install)
    }

    fn rollback(&mut self) -> Result<(), AdapterError> {
        let mut first_error = None;
        for part in &mut self.parts {
            if part.restored {
                continue;
            }
            let result = match &part.backup {
                Some(backup) => fs::rename(backup, &part.original),
                None => match fs::remove_file(&part.original) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error),
                },
            };
            match result {
                Ok(()) => part.restored = true,
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
        match first_error {
            Some(error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "wallet rollback failed; original database parts remain at {}: {error}",
                    self.rollback_directory.display()
                ),
            )
            .into()),
            None => {
                self.committed = true;
                let _ = fs::remove_dir_all(&self.rollback_directory);
                Ok(())
            }
        }
    }

    fn commit(mut self) {
        self.committed = true;
        let _ = fs::remove_dir_all(&self.rollback_directory);
    }
}

impl Drop for DatabaseInstall {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.rollback();
        }
    }
}

/// Reads recovery data without opening SQLite or contacting a server.
pub fn read_wallet_recovery_info(bytes: &[u8]) -> Result<String, AdapterError> {
    let decoded = DecodedWallet::decode(bytes)?;
    recovery_json(decoded.network, decoded.birthday, &decoded.seed_phrase)
}

/// Structurally validates checksummed Wcash wallet bytes.
pub fn validate_wallet_bytes(bytes: &[u8]) -> Result<(), AdapterError> {
    DecodedWallet::decode(bytes).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wcash_wallet::{derive_wallet_spending_key, encode_orchard_receiver};
    use zingolib::wcash::BlockRef;

    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    #[test]
    fn wallet_core_identity_comes_from_the_locked_git_source() {
        assert_eq!(WCASH_WALLET_CORE_REV.len(), 40);
        assert!(
            WCASH_WALLET_CORE_REV
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
        assert!(include_str!("../../Cargo.lock").contains(&format!(
            "wallet-core.git?rev={0}#{0}",
            WCASH_WALLET_CORE_REV
        )));
    }

    #[test]
    fn chain_hints_select_the_frozen_wcash_network() {
        assert_eq!(
            MobileNetwork::from_chain_hint("main").unwrap(),
            MobileNetwork::Mainnet
        );
        assert_eq!(
            MobileNetwork::from_chain_hint("test").unwrap(),
            MobileNetwork::Testnet
        );
        assert_eq!(
            MobileNetwork::from_chain_hint("regtest").unwrap(),
            MobileNetwork::Regtest
        );
        assert!(MobileNetwork::from_chain_hint("zcash").is_err());
        assert_eq!(MobileNetwork::Mainnet.ticker(), "WEC");
        assert_eq!(MobileNetwork::Testnet.ticker(), "TWC");
    }

    #[test]
    fn activation_heights_follow_the_locked_wallet_core() {
        for network in [
            MobileNetwork::Mainnet,
            MobileNetwork::Testnet,
            MobileNetwork::Regtest,
        ] {
            assert_eq!(
                network.wallet_activation_height(),
                consensus_wallet_activation_height(network.wallet_network())
            );
        }
    }

    #[test]
    fn synchronized_status_has_the_scanned_range_expected_by_upstream_ui() {
        assert_eq!(
            synchronized_scan_ranges(187, 426, true),
            vec![SyncScanRange {
                priority: "Scanned",
                start_block: 187,
                end_block: 426,
            }]
        );
        assert!(synchronized_scan_ranges(187, 425, false).is_empty());
    }

    #[tokio::test]
    async fn empty_walletless_endpoint_fails_closed() {
        assert!(matches!(
            WcashMobileAdapter::latest_server_height(" ").await,
            Err(AdapterError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    #[ignore = "requires the local Wcash Regtest indexer on 127.0.0.1:48234"]
    async fn local_regtest_endpoint_attests_and_reports_server_info() {
        let endpoint = "http://127.0.0.1:48234";
        let height = WcashMobileAdapter::latest_server_height(endpoint)
            .await
            .expect("the local endpoint must attest as Wcash");
        let info: serde_json::Value = serde_json::from_str(
            &WcashMobileAdapter::server_info_json(endpoint, MobileNetwork::Regtest)
                .await
                .expect("the local endpoint must return Wcash metadata"),
        )
        .unwrap();
        assert_eq!(info["chain_name"], "regtest");
        assert_eq!(info["latest_block_height"], height);
        assert_eq!(info["vendor"], "Wcash Wallet");
    }

    #[tokio::test]
    #[ignore = "requires the local Wcash Regtest indexer on 127.0.0.1:48234"]
    async fn local_regtest_first_slice_create_sync_save_and_restore() {
        let endpoint = "http://127.0.0.1:48234";
        let directory = tempfile::tempdir().unwrap();
        let wallet_path = directory.path().join("created.sqlite");
        let (mut wallet, recovery) =
            WcashMobileAdapter::create_new(endpoint, &wallet_path, MobileNetwork::Regtest)
                .await
                .expect("a local Wcash wallet must be created");
        let recovery: serde_json::Value = serde_json::from_str(&recovery).unwrap();
        assert_eq!(recovery["chain_name"], "regtest");
        let phrase = recovery["seed_phrase"].as_str().unwrap();
        assert_eq!(phrase.split_whitespace().count(), 24);

        // The unchanged React startup path reads these projections before its
        // first recovery sync. A new Wcash wallet must therefore return honest
        // empty state instead of recursing through a configuration failure.
        let starting_status: serde_json::Value =
            serde_json::from_str(&wallet.sync_status_json().unwrap()).unwrap();
        assert_eq!(starting_status["percentage_total_blocks_scanned"], 0);
        let starting_height: serde_json::Value =
            serde_json::from_str(&wallet.latest_wallet_block_json().unwrap()).unwrap();
        assert_eq!(starting_height["height"], 0);
        let starting_balance: serde_json::Value =
            serde_json::from_str(&wallet.balance_json().unwrap()).unwrap();
        assert_eq!(starting_balance["total_ironwood_balance"], 0);
        let starting_spendable: serde_json::Value =
            serde_json::from_str(&wallet.spendable_balance_json().unwrap()).unwrap();
        assert_eq!(starting_spendable["spendable_balance"], 0);
        let starting_history: serde_json::Value =
            serde_json::from_str(&wallet.confirmed_history_json(MAX_HISTORY_ROWS).unwrap())
                .unwrap();
        assert_eq!(starting_history["total"], 0);
        assert_eq!(starting_history["value_transfers"], serde_json::json!([]));

        wallet.prepare_sync();
        let sync: serde_json::Value =
            serde_json::from_str(&wallet.synchronize().await.unwrap()).unwrap();
        assert!(sync["sync_complete"]["sync_end_height"].is_number());
        let completed_status: serde_json::Value =
            serde_json::from_str(&wallet.sync_status_json().unwrap()).unwrap();
        assert_eq!(completed_status["scan_ranges"][0]["priority"], "Scanned");
        assert_eq!(
            completed_status["scan_ranges"][0]["start_block"],
            recovery["birthday"]
        );
        assert_eq!(
            completed_status["scan_ranges"][0]["end_block"],
            sync["sync_complete"]["sync_end_height"]
        );
        let balance: serde_json::Value =
            serde_json::from_str(&wallet.balance_json().unwrap()).unwrap();
        assert!(balance["total_ironwood_balance"].is_number());
        let addresses: serde_json::Value =
            serde_json::from_str(&wallet.unified_addresses_json().unwrap()).unwrap();
        let address = addresses[0]["encoded_address"].as_str().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&WcashMobileAdapter::parse_address_json(address).unwrap())
                .unwrap();
        assert_eq!(parsed["chain_name"], "regtest");
        assert!(matches!(
            wallet.confirm_send_json().await,
            Err(AdapterError::NoStagedSend)
        ));
        let history: serde_json::Value =
            serde_json::from_str(&wallet.confirmed_history_json(MAX_HISTORY_ROWS).unwrap())
                .unwrap();
        assert!(history["value_transfers"].is_array());

        let bytes = wallet.save_wallet_bytes().unwrap();
        validate_wallet_bytes(&bytes).unwrap();
        drop(wallet);
        let reopened = WcashMobileAdapter::open_wallet_bytes(
            &bytes,
            endpoint,
            directory.path().join("reopened.sqlite"),
            MobileNetwork::Regtest,
        )
        .await
        .expect("the checksummed wallet export must reopen");
        assert_eq!(reopened.network(), MobileNetwork::Regtest);

        let restored = WcashMobileAdapter::restore_from_seed(
            phrase,
            recovery["birthday"].as_u64().unwrap() as u32,
            endpoint,
            directory.path().join("restored.sqlite"),
            MobileNetwork::Regtest,
        )
        .await
        .expect("the recovery phrase must restore against local Wcash");
        assert_eq!(restored.0.network(), MobileNetwork::Regtest);
    }

    #[tokio::test]
    #[ignore = "requires the local Wcash Regtest indexer on 127.0.0.1:48234"]
    async fn rejected_restore_preserves_existing_database_and_sidecars() {
        let endpoint = "http://127.0.0.1:48234";
        let directory = tempfile::tempdir().unwrap();
        let wallet_path = directory.path().join("wcash-wallet.sqlite");
        let (mut wallet, _) =
            WcashMobileAdapter::create_new(endpoint, &wallet_path, MobileNetwork::Regtest)
                .await
                .unwrap();
        wallet.prepare_sync();
        wallet.synchronize().await.unwrap();
        let valid = wallet.save_wallet_bytes().unwrap();
        drop(wallet);
        let decoded = DecodedWallet::decode(&valid).unwrap();

        let wal = PathBuf::from(format!("{}-wal", wallet_path.display()));
        let shm = PathBuf::from(format!("{}-shm", wallet_path.display()));
        fs::write(&wal, b"sentinel-wal").unwrap();
        fs::write(&shm, b"sentinel-shm").unwrap();
        let before = [
            fs::read(&wallet_path).unwrap(),
            fs::read(&wal).unwrap(),
            fs::read(&shm).unwrap(),
        ];

        let mismatched_seed =
            encode_wallet(decoded.network, decoded.birthday, PHRASE, &decoded.database).unwrap();
        assert!(matches!(
            WcashMobileAdapter::open_wallet_bytes(
                &mismatched_seed,
                endpoint,
                &wallet_path,
                MobileNetwork::Regtest,
            )
            .await,
            Err(AdapterError::InvalidWalletBytes(_))
        ));
        assert_eq!(fs::read(&wallet_path).unwrap(), before[0]);
        assert_eq!(fs::read(&wal).unwrap(), before[1]);
        assert_eq!(fs::read(&shm).unwrap(), before[2]);

        let wrong_birthday = encode_wallet(
            decoded.network,
            decoded.birthday.saturating_add(1),
            &decoded.seed_phrase,
            &decoded.database,
        )
        .unwrap();
        assert!(matches!(
            WcashMobileAdapter::open_wallet_bytes(
                &wrong_birthday,
                endpoint,
                &wallet_path,
                MobileNetwork::Regtest,
            )
            .await,
            Err(AdapterError::InvalidWalletBytes(_))
        ));
        assert_eq!(fs::read(&wallet_path).unwrap(), before[0]);
        assert_eq!(fs::read(&wal).unwrap(), before[1]);
        assert_eq!(fs::read(&shm).unwrap(), before[2]);
    }

    #[test]
    fn wallet_envelope_round_trips_and_matches_native_plain_wallet_probe() {
        let database = b"SQLite format 3\0reviewed Wcash test database";
        let encoded = encode_wallet(MobileNetwork::Regtest, 42, PHRASE, database).unwrap();
        assert_eq!(u64::from_le_bytes(encoded[..8].try_into().unwrap()), 700);
        let decoded = DecodedWallet::decode(&encoded).unwrap();
        assert_eq!(decoded.network, MobileNetwork::Regtest);
        assert_eq!(decoded.birthday, 42);
        assert_eq!(decoded.seed_phrase, PHRASE);
        assert_eq!(decoded.database, database);
        let json: serde_json::Value =
            serde_json::from_str(&read_wallet_recovery_info(&encoded).unwrap()).unwrap();
        assert_eq!(json["seed_phrase"], PHRASE);
        assert_eq!(json["birthday"], 42);
        assert_eq!(json["chain_name"], "regtest");
    }

    #[test]
    fn wallet_envelope_rejects_tampering_and_trailing_bytes() {
        let mut encoded = encode_wallet(MobileNetwork::Testnet, 1, PHRASE, b"database").unwrap();
        encoded[WALLET_FILE_HEADER_LEN] ^= 1;
        assert!(matches!(
            validate_wallet_bytes(&encoded),
            Err(AdapterError::InvalidWalletBytes(_))
        ));
        let mut encoded = encode_wallet(MobileNetwork::Testnet, 1, PHRASE, b"database").unwrap();
        encoded.push(0);
        assert!(validate_wallet_bytes(&encoded).is_err());
    }

    #[test]
    fn failed_database_install_restores_main_wal_and_shm_byte_for_byte() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("wcash-wallet.sqlite");
        let wal = PathBuf::from(format!("{}-wal", destination.display()));
        let shm = PathBuf::from(format!("{}-shm", destination.display()));
        fs::write(&destination, b"original-main").unwrap();
        fs::write(&wal, b"original-wal").unwrap();
        fs::write(&shm, b"original-shm").unwrap();
        let validated = directory.path().join("validated.sqlite");
        fs::write(&validated, b"replacement-main").unwrap();

        let mut install = DatabaseInstall::begin(&validated, &destination).unwrap();
        fs::write(&wal, b"replacement-wal").unwrap();
        fs::write(&shm, b"replacement-shm").unwrap();
        install.rollback().unwrap();

        assert_eq!(fs::read(destination).unwrap(), b"original-main");
        assert_eq!(fs::read(wal).unwrap(), b"original-wal");
        assert_eq!(fs::read(shm).unwrap(), b"original-shm");
    }

    #[test]
    fn rollback_retries_only_unrestored_database_parts() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("wcash-wallet.sqlite");
        let wal = PathBuf::from(format!("{}-wal", destination.display()));
        let shm = PathBuf::from(format!("{}-shm", destination.display()));
        fs::write(&destination, b"original-main").unwrap();
        fs::write(&wal, b"original-wal").unwrap();
        fs::write(&shm, b"original-shm").unwrap();
        let validated = directory.path().join("validated.sqlite");
        fs::write(&validated, b"replacement-main").unwrap();

        let mut install = DatabaseInstall::begin(&validated, &destination).unwrap();
        fs::create_dir(&wal).unwrap();
        assert!(install.rollback().is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original-main");
        assert_eq!(fs::read(&shm).unwrap(), b"original-shm");

        fs::remove_dir(&wal).unwrap();
        install.rollback().unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"original-main");
        assert_eq!(fs::read(wal).unwrap(), b"original-wal");
        assert_eq!(fs::read(shm).unwrap(), b"original-shm");
    }

    #[test]
    fn persistent_rollback_failure_retains_the_only_backup_after_drop() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("wcash-wallet.sqlite");
        let wal = PathBuf::from(format!("{}-wal", destination.display()));
        let shm = PathBuf::from(format!("{}-shm", destination.display()));
        fs::write(&destination, b"original-main").unwrap();
        fs::write(&wal, b"original-wal").unwrap();
        fs::write(&shm, b"original-shm").unwrap();
        let validated = directory.path().join("validated.sqlite");
        fs::write(&validated, b"replacement-main").unwrap();

        let mut install = DatabaseInstall::begin(&validated, &destination).unwrap();
        let rollback_directory = install.rollback_directory.clone();
        fs::create_dir(&wal).unwrap();
        assert!(install.rollback().is_err());
        drop(install);

        assert_eq!(fs::read(&destination).unwrap(), b"original-main");
        assert_eq!(fs::read(&shm).unwrap(), b"original-shm");
        assert_eq!(
            fs::read(rollback_directory.join("original-1")).unwrap(),
            b"original-wal"
        );

        fs::remove_dir(&wal).unwrap();
        fs::rename(rollback_directory.join("original-1"), &wal).unwrap();
        fs::remove_dir_all(rollback_directory).unwrap();
        assert_eq!(fs::read(wal).unwrap(), b"original-wal");
    }

    #[test]
    fn install_start_failure_restores_existing_database_parts() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("wcash-wallet.sqlite");
        let wal = PathBuf::from(format!("{}-wal", destination.display()));
        let shm = PathBuf::from(format!("{}-shm", destination.display()));
        fs::write(&destination, b"original-main").unwrap();
        fs::write(&wal, b"original-wal").unwrap();
        fs::write(&shm, b"original-shm").unwrap();

        assert!(
            DatabaseInstall::begin(&directory.path().join("missing.sqlite"), &destination).is_err()
        );
        assert_eq!(fs::read(destination).unwrap(), b"original-main");
        assert_eq!(fs::read(wal).unwrap(), b"original-wal");
        assert_eq!(fs::read(shm).unwrap(), b"original-shm");
    }

    #[test]
    fn accepted_broadcast_is_success_and_blocks_replacement_after_local_write_failure() {
        let pending = PendingTransaction {
            request: PendingRequest::Shield,
            fee_zat: 10_000,
            value_to_shield_zat: Some(90_000),
            staged: None,
            calculated: None,
        };
        let mut slot = None;
        let txid = "ab".repeat(32);
        let response = accepted_broadcast_result(
            &mut slot,
            pending,
            txid.clone(),
            Err(AdapterError::Core("injected checkpoint failure".to_owned())),
        )
        .unwrap();
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["txids"][0], txid);
        assert!(slot.is_some(), "accepted send must keep its retry blocker");
    }

    #[test]
    fn invalid_seed_and_unbounded_database_are_rejected() {
        assert!(matches!(
            parse_seed("not a seed"),
            Err(AdapterError::InvalidSeed)
        ));
        assert!(encode_wallet(MobileNetwork::Testnet, 1, PHRASE, &[]).is_err());
    }

    #[test]
    fn parse_address_rejects_zcash_and_empty_addresses() {
        assert!(WcashMobileAdapter::parse_address_json("").is_err());
        assert!(WcashMobileAdapter::parse_address_json(
            "utest1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"
        )
        .is_err());
    }

    #[test]
    fn parse_address_accepts_each_wcash_network_and_reports_the_chain() {
        let mut addresses = Vec::new();
        for (network, mobile) in [
            (WalletNetwork::Mainnet, MobileNetwork::Mainnet),
            (WalletNetwork::Testnet, MobileNetwork::Testnet),
            (WalletNetwork::Regtest, MobileNetwork::Regtest),
        ] {
            let spending_key =
                derive_wallet_spending_key(&SecretVec::new(vec![31; 32]), network, 0).unwrap();
            let address =
                encode_orchard_receiver(&spending_key.to_unified_full_viewing_key(), network)
                    .unwrap();
            let value: serde_json::Value =
                serde_json::from_str(&WcashMobileAdapter::parse_address_json(&address).unwrap())
                    .unwrap();
            assert_eq!(value["chain_name"], mobile.chain_name());
            assert_eq!(value["address_kind"], "unified");
            assert_eq!(value["receivers_available"], serde_json::json!(["orchard"]));
            assert!(parse_address_for_network_json(&address, mobile).is_ok());
            addresses.push((mobile, address));
        }
        assert!(parse_address_for_network_json(&addresses[0].1, addresses[1].0).is_err());
        assert!(parse_address_for_network_json(&addresses[1].1, addresses[0].0).is_err());
    }

    #[test]
    fn metadata_less_history_uses_core_display_values_for_send_and_shield() {
        let history = ConfirmedTransactionSummaryHistory {
            exact_tip: BlockRef {
                height: 20,
                hash: [0; 32],
            },
            transactions: vec![
                ConfirmedTransactionSummary {
                    transaction: wcash_wallet::ConfirmedTransaction {
                        txid: "11".repeat(32),
                        mined_height: 19,
                        direction: ConfirmedTransactionDirection::Outgoing,
                        kind: ConfirmedTransactionKind::Transfer,
                        amount_delta_zat: -52,
                        fee_zat: Some(10),
                        timestamp: Some(1_700_000_000),
                        confirmations: 2,
                    },
                    value_zat: Some(42),
                },
                ConfirmedTransactionSummary {
                    transaction: wcash_wallet::ConfirmedTransaction {
                        txid: "22".repeat(32),
                        mined_height: 18,
                        direction: ConfirmedTransactionDirection::Internal,
                        kind: ConfirmedTransactionKind::Shielding,
                        amount_delta_zat: -10,
                        fee_zat: Some(10),
                        timestamp: Some(1_700_000_001),
                        confirmations: 3,
                    },
                    value_zat: Some(625_000_000),
                },
                ConfirmedTransactionSummary {
                    transaction: wcash_wallet::ConfirmedTransaction {
                        txid: "33".repeat(32),
                        mined_height: 17,
                        direction: ConfirmedTransactionDirection::Outgoing,
                        kind: ConfirmedTransactionKind::Transfer,
                        amount_delta_zat: -10_000,
                        fee_zat: Some(10_000),
                        timestamp: Some(1_700_000_002),
                        confirmations: 4,
                    },
                    value_zat: None,
                },
            ],
        };
        let mut mapped = mobile_history(history);
        assert_eq!(mapped.total, 3);
        assert_eq!(mapped.value_transfers[0].kind, "sent");
        assert_eq!(mapped.value_transfers[0].value, 42);
        assert_eq!(mapped.value_transfers[0].pools_sent_from, vec!["Ironwood"]);
        assert!(mapped.value_transfers[0].pools_received.is_empty());
        assert_eq!(mapped.value_transfers[1].kind, "shield");
        assert_eq!(mapped.value_transfers[1].value, 625_000_000);
        assert_eq!(
            mapped.value_transfers[1].pools_sent_from,
            vec!["Transparent"]
        );
        assert_eq!(mapped.value_transfers[1].pools_received, vec!["Ironwood"]);
        assert_eq!(mapped.value_transfers[2].kind, "unknown");
        assert_eq!(mapped.value_transfers[2].value, 0);
        assert_eq!(mapped.value_transfers[2].transaction_fee, None);
        assert!(mapped.value_transfers[2].pools_sent_from.is_empty());
        assert!(mapped.value_transfers[2].pools_received.is_empty());

        let shield = PendingMetadata {
            txid: "11".repeat(32),
            kind: "shield".to_owned(),
            value_zat: 625_000_000,
            fee_zat: 15_000,
            state: "transmitted".to_owned(),
            recipient_address: None,
            created_at: 1_700_000_000,
        };
        shield
            .apply_to_confirmed(&mut mapped.value_transfers[2])
            .unwrap();
        assert_eq!(mapped.value_transfers[2].kind, "shield");
        assert_eq!(mapped.value_transfers[2].value, 625_000_000);
        assert_eq!(
            mapped.value_transfers[2].pools_sent_from,
            vec!["Transparent"]
        );
        assert_eq!(mapped.value_transfers[2].pools_received, vec!["Ironwood"]);
    }
}
