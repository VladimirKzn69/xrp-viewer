// use reqwest::{Client, Error as ReqwestError};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

use crate::models::{
    AccountInfoRequest, AccountInfoResponse, AccountTxRequest, AccountTxResponse,
    DisplayTransaction,
};

use anyhow::{Context, Result};

pub struct XrpApi {
    client: Client,
    base_url: String,
}

impl XrpApi {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("xrp-viewer/0.1.0")
            .build()
            .context("Не удалось создать HTTP-клиент")?;

        Ok(XrpApi {
            client,
            base_url: "https://s1.ripple.com:51234  ".to_string(),
        })
    }

    pub async fn get_account_info(&self, address: &str) -> Result<AccountInfoResponse> {
        let request = AccountInfoRequest::new(address.to_string());

        log::debug!("Отправка запроса account_info для адреса: {}", address);

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .context("Не удалось отправить запрос к API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Неизвестная ошибка".to_string());
            log::error!("API вернул ошибку {}: {}", status, error_text);
            anyhow::bail!("API вернул ошибку {}: {}", status, error_text);
        }

        let account_info: AccountInfoResponse = response
            .json()
            .await
            .context("Не удалось разобрать ответ API")?;

        if account_info.result.status != "success" {
            log::error!("API вернул статус: {}", account_info.result.status);
            anyhow::bail!("API вернул статус: {}", account_info.result.status);
        }

        log::debug!("Получен ответ account_info для адреса: {}", address);
        Ok(account_info)
    }

    pub async fn get_latest_transaction(
        &self,
        address: &str,
    ) -> Result<Option<DisplayTransaction>> {
        let request = AccountTxRequest::new(address.to_string());

        log::debug!("Отправка запроса account_tx для адреса: {}", address);

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .context("Не удалось отправить запрос к API")?;

        if !response.status().is_success() {
            let status = response.status();
            log::error!("API вернул ошибку: {}", status);
            anyhow::bail!("API вернул ошибку: {}", status);
        }

        let account_tx: AccountTxResponse = response
            .json()
            .await
            .context("Не удалось разобрать ответ API")?;

        if account_tx.result.status != "success" {
            log::error!("API вернул статус: {}", account_tx.result.status);
            anyhow::bail!("API вернул статус: {}", account_tx.result.status);
        }

        if account_tx.result.transactions.is_empty() {
            log::debug!("У кошелька нет транзакций");
            return Ok(None);
        }

        let first_tx_wrapper = &account_tx.result.transactions[0];
        let transaction = &first_tx_wrapper.tx;

        let display_tx = DisplayTransaction::from_transaction(transaction);

        log::debug!("Получена последняя транзакция для адреса: {}", address);
        Ok(display_tx)
    }

    #[allow(dead_code)]
    pub async fn send_json_rpc_request<T>(&self, request_body: &Value) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self
            .client
            .post(&self.base_url)
            .json(request_body)
            .send()
            .await
            .context("Не удалось отправить запрос к API")?;

        if !response.status().is_success() {
            let status = response.status();
            log::error!("API вернул ошибку: {}", status);
            anyhow::bail!("API вернул ошибку: {}", status);
        }

        let result: T = response
            .json()
            .await
            .context("Не удалось разобрать ответ API")?;

        Ok(result)
    }
}

impl Default for XrpApi {
    fn default() -> Self {
        Self::new().expect("Не удалось создать API-клиент")
    }
}

#[allow(dead_code)]
pub async fn is_account_exists(api: &XrpApi, address: &str) -> Result<bool> {
    match api.get_account_info(address).await {
        Ok(_) => Ok(true),
        Err(e) => {
            if e.to_string().contains("actNotFound") || e.to_string().contains("Account not found")
            {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}
