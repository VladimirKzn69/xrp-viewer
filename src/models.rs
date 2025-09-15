use serde::{Deserialize, Serialize};

// --- Структуры для account_info ---
#[derive(Serialize, Debug)]
pub struct AccountInfoRequest {
    pub method: String,
    pub params: Vec<AccountInfoParams>,
}

#[derive(Serialize, Debug)]
pub struct AccountInfoParams {
    pub account: String,
    pub strict: bool,
}

impl AccountInfoRequest {
    pub fn new(account: String) -> Self {
        AccountInfoRequest {
            method: "account_info".to_string(),
            params: vec![AccountInfoParams {
                account,
                strict: true,
            }],
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct AccountInfoResponse {
    pub result: AccountInfoResult,
}

#[derive(Deserialize, Debug)]
pub struct AccountInfoResult {
    pub account_data: AccountData,
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct AccountData {
    #[serde(rename = "Account")]
    pub account: String,
    #[serde(rename = "Balance")]
    pub balance: String,
    #[serde(rename = "Flags")]
    pub flags: u32,
    #[serde(rename = "LedgerEntryType")]
    pub ledger_entry_type: String,
    #[serde(rename = "OwnerCount")]
    pub owner_count: u32,
    #[serde(rename = "PreviousTxnID")]
    pub previous_txn_id: String,
    #[serde(rename = "PreviousTxnLgrSeq")]
    pub previous_txn_lgr_seq: u32,
    #[serde(rename = "Sequence")]
    pub sequence: u32,
    #[serde(rename = "index")]
    pub index: String,
}

impl AccountData {
    pub fn balance_xrp(&self) -> f64 {
        self.balance.parse::<f64>().unwrap_or(0.0) / 1_000_000.0
    }
}

// --- Структуры для account_tx ---
#[derive(Serialize, Debug)]
pub struct AccountTxRequest {
    pub method: String,
    pub params: Vec<AccountTxParams>,
}

#[derive(Serialize, Debug)]
pub struct AccountTxParams {
    pub account: String,
    pub ledger_index_min: i32,
    pub ledger_index_max: i32,
    pub limit: u32,
}

impl AccountTxRequest {
    pub fn new(account: String) -> Self {
        AccountTxRequest {
            method: "account_tx".to_string(),
            params: vec![AccountTxParams {
                account,
                ledger_index_min: -1,
                ledger_index_max: -1,
                limit: 5,
            }],
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct AccountTxResponse {
    pub result: AccountTxResult,
}

#[derive(Deserialize, Debug)]
pub struct AccountTxResult {
    pub account: String,
    pub transactions: Vec<TransactionWrapper>,
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct TransactionWrapper {
    pub tx: Transaction,
    pub meta: TransactionMeta,
    pub validated: bool,
}

#[derive(Deserialize, Debug)]
pub struct Transaction {
    #[serde(rename = "Account")]
    pub account: String,
    #[serde(rename = "Amount", skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(rename = "Destination", skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(rename = "Fee")]
    pub fee: String,
    #[serde(rename = "TransactionType")]
    pub transaction_type: String,
    #[serde(rename = "hash")]
    pub hash: String,
    #[serde(rename = "date", skip_serializing_if = "Option::is_none")]
    pub date: Option<u64>,
}

impl Transaction {
    pub fn amount_xrp(&self) -> f64 {
        self.amount
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1_000_000.0
    }

    pub fn formatted_date(&self) -> String {
        match self.date {
            Some(d) => format_xrp_timestamp(d),
            None => "Неизвестно".to_string(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct TransactionMeta {
    #[serde(rename = "TransactionResult")]
    pub transaction_result: String,
}

fn format_xrp_timestamp(xrp_timestamp: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const XRP_EPOCH_OFFSET: u64 = 946684800;

    let unix_timestamp = xrp_timestamp + XRP_EPOCH_OFFSET;
    let datetime = UNIX_EPOCH + Duration::from_secs(unix_timestamp);

    match datetime.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let total_seconds = duration.as_secs();
            let days = total_seconds / 86400;
            let hours = (total_seconds % 86400) / 3600;
            let minutes = (total_seconds % 3600) / 60;

            format!(
                "20{}-{:02}-{:02} {:02}:{:02} UTC",
                24 + (days / 365),
                ((days % 365) / 30) + 1,
                (days % 30) + 1,
                hours,
                minutes
            )
        }
        Err(_) => "Некорректная дата".to_string(),
    }
}

// --- Структуры для отображения ---
#[derive(Debug, Clone)]
pub struct DisplayAccountInfo {
    pub address: String,
    pub balance_xrp: f64,
}

impl DisplayAccountInfo {
    pub fn from_account_data(address: String, account_data: &AccountData) -> Self {
        DisplayAccountInfo {
            address,
            balance_xrp: account_data.balance_xrp(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayTransaction {
    pub hash: String,
    pub amount_xrp: f64,
    pub timestamp: String,
    pub from: String,
    pub to: String,
    pub network: Option<String>,
}

impl DisplayTransaction {
    pub fn from_transaction(tx: &Transaction) -> Option<Self> {
        if tx.transaction_type == "Payment" {
            Some(DisplayTransaction {
                hash: tx.hash.clone(),
                amount_xrp: tx.amount_xrp(),
                timestamp: tx.formatted_date(),
                from: tx.account.clone(),
                to: tx
                    .destination
                    .clone()
                    .unwrap_or_else(|| "Неизвестно".to_string()),
                network: None,
            })
        } else {
            None
        }
    }
}

// --- Структуры для транзакций ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCommonFields {
    #[serde(rename = "TransactionType")]
    pub transaction_type: String,
    #[serde(rename = "Account")]
    pub account: String,
    #[serde(rename = "Fee")]
    pub fee: String,
    #[serde(rename = "Sequence")]
    pub sequence: u32,
    #[serde(rename = "LastLedgerSequence", skip_serializing_if = "Option::is_none")]
    pub last_ledger_sequence: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentFields {
    #[serde(rename = "Amount")]
    pub amount: String,
    #[serde(rename = "Destination")]
    pub destination: String,
}

// --- Структуры для server_state ---
#[derive(Serialize, Debug)]
pub struct ServerStateRequest {
    pub method: String,
    pub params: Vec<serde_json::Value>,
}

impl ServerStateRequest {
    pub fn new() -> Self {
        ServerStateRequest {
            method: "server_state".to_string(),
            params: vec![],
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ServerStateResponse {
    pub result: ServerStateResult,
}

#[derive(Deserialize, Debug)]
pub struct ServerStateResult {
    pub state: ServerState,
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct ServerState {
    pub server_state: String,
    pub validated_ledger: ValidatedLedger,
}

#[derive(Deserialize, Debug)]
pub struct ValidatedLedger {
    pub seq: u32,
}

// --- Структуры для submit ---
#[derive(Serialize, Debug)]
pub struct SubmitRequest {
    pub method: String,
    pub params: Vec<SubmitParams>,
}

#[derive(Serialize, Debug)]
pub struct SubmitParams {
    pub tx_blob: String,
}

impl SubmitRequest {
    pub fn new(tx_blob: String) -> Self {
        SubmitRequest {
            method: "submit".to_string(),
            params: vec![SubmitParams { tx_blob }],
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct SubmitResponse {
    pub result: SubmitResult,
}

#[derive(Deserialize, Debug)]
pub struct SubmitResult {
    pub engine_result: String,
    pub engine_result_code: i32,
    pub engine_result_message: String,
    pub status: String,
    pub tx_blob: String,
    pub tx_json: serde_json::Value,
}

// --- Структуры для Faucet (НОВОЕ) ---
#[derive(Debug, Deserialize)]
pub struct FaucetResponse {
    pub account: String,
    pub amount: String,
    #[serde(default)]
    pub balance: Option<String>,
    pub status: String,
    #[serde(default)]
    pub tx_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FaucetRequest {
    pub destination: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
}
