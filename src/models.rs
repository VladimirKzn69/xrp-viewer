use serde::{Deserialize, Serialize};
use std::collections::BTreeMap; // Добавлено для Transaction

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

#[derive(Deserialize, Debug, Clone)]
pub struct AccountInfoResponse {
    pub result: AccountInfoResult,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountInfoResult {
    pub account_data: AccountData,
    pub status: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountData {
    #[serde(rename = "Balance")]
    pub balance: String,
    // Добавляем sequence
    #[serde(rename = "Sequence")]
    pub sequence: Option<u32>, // Может отсутствовать у неактивированных аккаунтов
    // #[serde(rename = "Account")]
    // pub account: String,
}

impl AccountData {
    pub fn balance_xrp(&self) -> f64 {
        match self.balance.parse::<f64>() {
            Ok(balance_drops) => balance_drops / 1_000_000.0,
            Err(_) => 0.0,
        }
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
    pub limit: u32,
    pub descending: bool,
}

impl AccountTxRequest {
    pub fn new(account: String) -> Self {
        AccountTxRequest {
            method: "account_tx".to_string(),
            params: vec![AccountTxParams {
                account,
                limit: 1,
                descending: true,
            }],
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountTxResponse {
    pub result: AccountTxResult,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountTxResult {
    pub transactions: Vec<TransactionWrapper>,
    pub status: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TransactionWrapper {
    pub tx: Transaction,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Transaction {
    pub hash: String,
    #[serde(rename = "Amount")]
    pub amount: Option<String>,
    pub date: Option<u64>,
    #[serde(rename = "Account")]
    pub account: String,
    #[serde(rename = "Destination")]
    pub destination: Option<String>,
    #[serde(rename = "TransactionType")]
    pub transaction_type: String,
}

impl Transaction {
    pub fn amount_xrp(&self) -> f64 {
        match &self.amount {
            Some(amount_str) => match amount_str.parse::<f64>() {
                Ok(amount_drops) => amount_drops / 1_000_000.0,
                Err(_) => 0.0,
            },
            None => 0.0,
        }
    }
    pub fn formatted_date(&self) -> String {
        match self.date {
            Some(timestamp) => {
                let ripple_epoch = 946_684_800;
                let unix_timestamp = ripple_epoch + timestamp;
                format_timestamp(unix_timestamp)
            }
            None => "Нет данных".to_string(),
        }
    }
}

fn format_timestamp(timestamp: u64) -> String {
    // use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use std::time::{Duration, UNIX_EPOCH};
    let dt = UNIX_EPOCH + Duration::from_secs(timestamp);
    match dt.duration_since(UNIX_EPOCH) {
        Ok(_) => {
            let secs = timestamp;
            let days = secs / 86400;
            let hours = (secs % 86400) / 3600;
            let minutes = (secs % 3600) / 60;
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
            })
        } else {
            None
        }
    }
}

// --- Добавленные структуры для транзакции (учебный проект) ---

// Базовая структура транзакции XRP
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

// Специфичные поля для транзакции Payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentFields {
    #[serde(rename = "Amount")]
    pub amount: String,
    #[serde(rename = "Destination")]
    pub destination: String,
}

// --- Добавленные структуры для server_state ---
#[derive(Serialize, Debug)]
pub struct ServerStateRequest {
    pub method: String,
    pub params: Vec<serde_json::Value>,
}

impl ServerStateRequest {
    pub fn new() -> Self {
        ServerStateRequest {
            method: "server_state".to_string(),
            params: vec![], // Обычно без параметров
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ServerStateResponse {
    pub result: ServerStateResult,
}

#[derive(Deserialize, Debug)]
pub struct ServerStateResult {
    pub state: ServerStateInfo,
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct ServerStateInfo {
    #[serde(rename = "validated_ledger")]
    pub validated_ledger: ValidatedLedgerInfo,
}

#[derive(Deserialize, Debug)]
pub struct ValidatedLedgerInfo {
    #[serde(rename = "seq")] // Или ledger_index
    pub ledger_index: Option<u32>,
    #[serde(rename = "base_fee_xrp")]
    pub base_fee_xrp: Option<String>,
}

// --- Добавленные структуры для submit ---
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
    pub tx_blob: Option<String>,
    #[serde(rename = "tx_json")]
    pub tx_json: Option<serde_json::Value>,
    pub hash: Option<String>,
    pub status: String,
}

impl SubmitResult {
    // Вспомогательный метод для получения хэша
    pub fn get_transaction_hash(&self) -> Option<String> {
        // Приоритет: hash -> tx_json.hash -> tx_blob (нужно парсить)
        if let Some(hash) = &self.hash {
            Some(hash.clone())
        } else if let Some(tx_json) = &self.tx_json {
             if let Some(hash_val) = tx_json.get("hash") {
                hash_val.as_str().map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}
