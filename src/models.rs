use serde::{Deserialize, Serialize};

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
            Some(amount_str) => {
                match amount_str.parse::<f64>() {
                    Ok(amount_drops) => amount_drops / 1_000_000.0,
                    Err(_) => 0.0,
                }
            }
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
            
            format!("20{}-{:02}-{:02} {:02}:{:02} UTC", 
                   24 + (days / 365), 
                   ((days % 365) / 30) + 1, 
                   (days % 30) + 1,
                   hours, 
                   minutes)
        }
        Err(_) => "Некорректная дата".to_string(),
    }
}

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
                to: tx.destination.clone().unwrap_or_else(|| "Неизвестно".to_string()),
            })
        } else {
            None
        }
    }
}