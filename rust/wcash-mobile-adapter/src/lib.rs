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
use secrecy::{ExposeSecret, SecretString, SecretVec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wcash_wallet::{
    AttestedWcashClient, MAX_CONFIRMED_TRANSACTION_HISTORY_SIZE, WalletNetwork, decode_recipient,
};
pub use zingolib::wcash::WalletSyncCancellation;
#[cfg(feature = "regtest")]
use zingolib::wcash::WcashRegtestRuntime;
use zingolib::wcash::{
    ConfirmedTransaction, ConfirmedTransactionDirection, ConfirmedTransactionHistory,
    ConfirmedTransactionKind, InitializedWallet, SignedTransaction, WalletBalanceSummary,
    WalletInfo, WcashTestnetPayment, WcashTestnetRuntime,
};

const WALLET_FILE_VERSION: u64 = 700;
const WALLET_FILE_MAGIC: &[u8; 8] = b"WCASHM01";
const WALLET_FILE_HEADER_LEN: usize = 8 + 8 + 1 + 4 + 4 + 8 + 32;
const MAX_SEED_PHRASE_BYTES: usize = 512;
const MAX_WALLET_DATABASE_BYTES: usize = 512 * 1024 * 1024;
const MAX_HISTORY_ROWS: usize = MAX_CONFIRMED_TRANSACTION_HISTORY_SIZE;
const MINIMUM_PREVIEW_FEE_ZAT: u64 = 10_000;

/// Networks implemented by the reviewed Wcash backend.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MobileNetwork {
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
            "main" => Err(AdapterError::UnsupportedMainnet),
            _ => Err(AdapterError::InvalidInput("unknown chain hint".to_owned())),
        }
    }

    /// Returns the short chain token expected by the React Native app.
    pub const fn chain_name(self) -> &'static str {
        match self {
            Self::Testnet => "test",
            Self::Regtest => "regtest",
        }
    }

    /// Returns the test-funds ticker used by both available runtimes.
    pub const fn ticker(self) -> &'static str {
        "TWC"
    }

    fn wallet_network(self) -> WalletNetwork {
        match self {
            Self::Testnet => WalletNetwork::Testnet,
            Self::Regtest => WalletNetwork::Regtest,
        }
    }

    fn envelope_byte(self) -> u8 {
        match self {
            Self::Testnet => 1,
            Self::Regtest => 2,
        }
    }

    fn from_envelope_byte(value: u8) -> Result<Self, AdapterError> {
        match value {
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
    /// Wcash Mainnet is intentionally unavailable until its consensus identity is frozen.
    #[error("Wcash Mainnet is not available in this build")]
    UnsupportedMainnet,
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
            Self::Testnet(runtime) => runtime.sync(cancellation).await.map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime.sync(cancellation).await.map_err(core_error),
        }
    }

    fn balance(&self) -> Result<WalletBalanceSummary, AdapterError> {
        match self {
            Self::Testnet(runtime) => runtime.balance().map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime.balance().map_err(core_error),
        }
    }

    fn receive(&self) -> Result<WalletInfo, AdapterError> {
        match self {
            Self::Testnet(runtime) => {
                WcashTestnetRuntime::inspect(runtime.wallet_path()).map_err(core_error)
            }
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => {
                WcashRegtestRuntime::inspect(runtime.wallet_path()).map_err(core_error)
            }
        }
    }

    fn history(&self, limit: usize) -> Result<ConfirmedTransactionHistory, AdapterError> {
        match self {
            Self::Testnet(runtime) => runtime.confirmed_transactions(limit).map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime.confirmed_transactions(limit).map_err(core_error),
        }
    }

    async fn send(
        &mut self,
        seed: &SecretVec<u8>,
        payments: Vec<WcashTestnetPayment>,
    ) -> Result<SignedTransaction, AdapterError> {
        match self {
            Self::Testnet(runtime) => runtime.send(seed, payments).await.map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime.send(seed, payments).await.map_err(core_error),
        }
    }

    async fn broadcast(&mut self, signed: &SignedTransaction) -> Result<String, AdapterError> {
        match self {
            Self::Testnet(runtime) => runtime
                .broadcast(signed)
                .await
                .map(|result| result.txid)
                .map_err(core_error),
            #[cfg(feature = "regtest")]
            Self::Regtest(runtime) => runtime
                .broadcast(signed)
                .await
                .map(|result| result.txid)
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
    pending_send: Option<PendingSend>,
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
        for network in [MobileNetwork::Regtest, MobileNetwork::Testnet] {
            match AttestedWcashClient::connect(endpoint, network.wallet_network()).await {
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
            "endpoint did not attest as Wcash Testnet or Regtest ({})",
            errors.join("; ")
        )))
    }

    /// Returns the server metadata shape consumed by the unchanged mobile UI.
    pub async fn server_info_json(
        endpoint: &str,
        network: MobileNetwork,
    ) -> Result<String, AdapterError> {
        let wallet_network = network.wallet_network();
        let mut client = AttestedWcashClient::connect(endpoint, wallet_network)
            .await
            .map_err(core_error)?;
        let latest = client.latest_block().await.map_err(core_error)?;
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "version": "0.1.0",
            "git_commit": "58bc22e",
            "server_uri": endpoint,
            "vendor": "Wcash Wallet",
            "taddr_support": true,
            "chain_name": network.chain_name(),
            "sapling_activation_height": 1,
            "consensus_branch_id": wallet_network.branch_id_hex(),
            "latest_block_height": latest.height,
            "ironwood_activation_height": 1
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
        write_wallet_database(&wallet_path, &decoded.database)?;
        let (runtime, info) = Runtime::open(expected_network, endpoint, &wallet_path).await?;
        if info.birthday_height != decoded.birthday {
            return Err(AdapterError::InvalidWalletBytes(
                "wallet birthday does not match its database".to_owned(),
            ));
        }
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
            scan_ranges: Vec::new(),
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
        let network = [MobileNetwork::Testnet, MobileNetwork::Regtest]
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

    /// Returns bounded, newest-first confirmed history in the upstream list shape.
    pub fn confirmed_history_json(&self, limit: usize) -> Result<String, AdapterError> {
        if limit == 0 || limit > MAX_HISTORY_ROWS {
            return Err(AdapterError::InvalidInput(format!(
                "history limit must be between 1 and {MAX_HISTORY_ROWS}"
            )));
        }
        let history = match self.runtime.history(limit) {
            Ok(history) => mobile_history(history),
            Err(error) if recovery_is_incomplete(&error) => MobileHistory {
                value_transfers: Vec::new(),
                total: 0,
            },
            Err(error) => return Err(error),
        };
        Ok(serde_json::to_string_pretty(&history)?)
    }

    /// Validates and prepares a side-effect-free Wcash send preview.
    ///
    /// Zingo Mobile invokes this contract repeatedly as the form changes and
    /// once more after the user accepts the confirmation sheet. The Wcash core
    /// has no pure proposal API, so this returns ZIP 317's minimum preview fee;
    /// [`Self::confirm_send_json`] verifies the signed fee before broadcasting.
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
            payments.push(WcashTestnetPayment {
                address: request.address,
                amount_zat: request.amount,
                memo,
            });
        }

        if let Some(pending) = &self.pending_send
            && pending.payments != payments
            && pending.signed.is_some()
        {
            return Err(AdapterError::SendAlreadyStaged);
        }
        let fee = self
            .pending_send
            .as_ref()
            .filter(|pending| pending.payments == payments)
            .and_then(|pending| pending.signed.as_ref())
            .map_or(MINIMUM_PREVIEW_FEE_ZAT, |signed| signed.fee_zat);
        match &mut self.pending_send {
            Some(pending) if pending.payments == payments => {
                pending.preview_fee_zat = fee;
            }
            _ => {
                self.pending_send = Some(PendingSend {
                    payments,
                    preview_fee_zat: fee,
                    signed: None,
                });
            }
        }
        let response = SendProposal {
            fee,
            source_pools: vec!["ironwood"],
            destination_pools: vec!["ironwood"],
        };
        Ok(serde_json::to_string_pretty(&response)?)
    }

    /// Signs after consent and broadcasts only when the previewed fee is exact.
    ///
    /// If the actual fee is above the minimum preview, the signed bytes stay
    /// staged and this call fails closed. The unchanged UI's next preview then
    /// shows that exact fee, and its next confirm retries the same signed bytes.
    pub async fn confirm_send_json(&mut self) -> Result<String, AdapterError> {
        let mut pending = self.pending_send.take().ok_or(AdapterError::NoStagedSend)?;
        let signed = match pending.signed.clone() {
            Some(signed) => signed,
            None => {
                let mnemonic = parse_seed(self.seed_phrase.expose_secret())?;
                let seed = SecretVec::new(mnemonic.to_seed("").to_vec());
                match self.runtime.send(&seed, pending.payments.clone()).await {
                    Ok(signed) => signed,
                    Err(error) => {
                        self.pending_send = Some(pending);
                        return Err(error);
                    }
                }
            }
        };
        pending.signed = Some(signed.clone());
        if signed.fee_zat != pending.preview_fee_zat {
            self.pending_send = Some(pending);
            return Err(AdapterError::UnsupportedFeature(
                "the exact signed fee differs from the send preview; review it again",
            ));
        }
        let txid = match self.runtime.broadcast(&signed).await {
            Ok(txid) => txid,
            Err(error) => {
                self.pending_send = Some(pending);
                return Err(error);
            }
        };
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "txids": [txid]
        }))?)
    }

    /// Discards a side-effect-free preview. Signed retries remain staged.
    pub fn discard_staged_send(&mut self) {
        if self
            .pending_send
            .as_ref()
            .is_some_and(|pending| pending.signed.is_none())
        {
            self.pending_send = None;
        }
    }

    /// Serializes the seed plus SQLite database into bounded checksummed wallet bytes.
    pub fn save_wallet_bytes(&self) -> Result<Vec<u8>, AdapterError> {
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
}

fn mobile_history(history: ConfirmedTransactionHistory) -> MobileHistory {
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

fn mobile_value_transfer(tx: ConfirmedTransaction) -> MobileValueTransfer {
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
        value: tx.amount_delta_zat.unsigned_abs(),
        pools_sent_from,
        pools_received,
    }
}

#[derive(Deserialize)]
struct MobilePayment {
    address: String,
    amount: u64,
    #[serde(default)]
    memo: Option<String>,
}

struct PendingSend {
    payments: Vec<WcashTestnetPayment>,
    preview_fee_zat: u64,
    signed: Option<SignedTransaction>,
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
    scan_ranges: Vec<serde_json::Value>,
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

fn write_wallet_database(path: &Path, database: &[u8]) -> Result<(), AdapterError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("wcash-restore.tmp");
    fs::write(&temporary, database)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&temporary, path)?;
    Ok(())
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
    use wcash_wallet::{
        AccountBalanceSummary, BlockRef, derive_wallet_spending_key, encode_orchard_receiver,
    };

    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    #[test]
    fn chain_hints_fail_closed_without_mainnet() {
        assert_eq!(
            MobileNetwork::from_chain_hint("test").unwrap(),
            MobileNetwork::Testnet
        );
        assert_eq!(
            MobileNetwork::from_chain_hint("regtest").unwrap(),
            MobileNetwork::Regtest
        );
        assert!(matches!(
            MobileNetwork::from_chain_hint("main"),
            Err(AdapterError::UnsupportedMainnet)
        ));
        assert!(MobileNetwork::from_chain_hint("zcash").is_err());
        assert_eq!(MobileNetwork::Testnet.ticker(), "TWC");
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
        let first_preview: serde_json::Value = serde_json::from_str(
            &wallet
                .stage_send_json(&format!(r#"[{{"address":"{address}","amount":1}}]"#))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(first_preview["fee"], MINIMUM_PREVIEW_FEE_ZAT);
        let changed_preview: serde_json::Value = serde_json::from_str(
            &wallet
                .stage_send_json(&format!(r#"[{{"address":"{address}","amount":2}}]"#))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(changed_preview["fee"], MINIMUM_PREVIEW_FEE_ZAT);
        wallet.discard_staged_send();
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
    fn balance_mapping_keeps_confirmed_locked_value_and_pending_value_separate() {
        let summary = WalletBalanceSummary {
            chain_tip_height: 50,
            fully_scanned_height: 50,
            synchronized: true,
            accounts: vec![AccountBalanceSummary {
                account_id: "0".to_owned(),
                ironwood_total_zat: 100,
                ironwood_spendable_zat: 60,
                ironwood_locked_zat: 10,
                ironwood_pending_change_zat: 20,
                ironwood_pending_spendability_zat: 10,
                sapling_total_zat: 0,
                orchard_total_zat: 0,
                transparent_total_zat: 80,
                transparent_coinbase_total_zat: 80,
                transparent_coinbase_spendable_zat: 50,
                transparent_coinbase_pending_zat: 30,
                transparent_regular_total_zat: 0,
            }],
        };
        let mapped = mobile_balance(&summary);
        assert_eq!(mapped.total_ironwood_balance, 100);
        assert_eq!(mapped.confirmed_ironwood_balance, 70);
        assert_eq!(mapped.unconfirmed_ironwood_balance, 30);
        assert_eq!(mapped.confirmed_transparent_balance, 50);
        assert_eq!(mapped.unconfirmed_transparent_balance, 30);
    }

    #[test]
    fn history_mapping_uses_existing_mobile_kinds_and_ironwood_pool_name() {
        let history = ConfirmedTransactionHistory {
            exact_tip: BlockRef {
                height: 20,
                hash: [0; 32],
            },
            transactions: vec![ConfirmedTransaction {
                txid: "11".repeat(32),
                mined_height: 19,
                direction: ConfirmedTransactionDirection::Outgoing,
                kind: ConfirmedTransactionKind::Transfer,
                amount_delta_zat: -42,
                fee_zat: Some(10_000),
                timestamp: Some(1_700_000_000),
                confirmations: 2,
            }],
        };
        let mapped = mobile_history(history);
        assert_eq!(mapped.total, 1);
        assert_eq!(mapped.value_transfers[0].kind, "sent");
        assert_eq!(mapped.value_transfers[0].value, 42);
        assert_eq!(mapped.value_transfers[0].pools_sent_from, vec!["Ironwood"]);
        assert!(mapped.value_transfers[0].pools_received.is_empty());
    }
}
