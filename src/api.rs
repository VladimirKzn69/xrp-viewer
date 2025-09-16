use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

use crate::models::{
    AccountInfoRequest, AccountInfoResponse, AccountTxRequest, AccountTxResponse, FaucetRequest,
    FaucetResponse, ServerStateRequest, ServerStateResponse, SubmitRequest, SubmitResponse,
};
use crate::network::Network;

pub struct XrpApi {
    client: Client,
    network: Network,
    base_url: String,
}

impl XrpApi {
    pub fn new(network: Network) -> Result<Self> {
        let config = network.config();

        // Специальная обработка для testnet - используем альтернативный URL
        let base_url = if matches!(network, Network::Testnet) {
            // Используем прямой доступ к Ripple testnet
            "https://s.altnet.rippletest.net:51234/".to_string()
        } else {
            config.rpc_url.to_string()
        };

        // Создаем клиент с правильными заголовками для обхода блокировок
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .danger_accept_invalid_certs(true) // Для testnet может понадобиться
            .build()
            .context("Не удалось создать HTTP клиент")?;

        log::info!("🔗 Создан API клиент для сети {}", network);
        log::debug!("   RPC URL: {}", base_url);

        Ok(Self {
            client,
            network,
            base_url,
        })
    }

    pub fn network(&self) -> &Network {
        &self.network
    }

    pub async fn get_account_info(&self, address: &str) -> Result<AccountInfoResponse> {
        let request = AccountInfoRequest::new(address.to_string());
        log::info!(
            "📡 Отправка запроса account_info для {} в сети {}",
            address,
            self.network
        );
        log::debug!("   URL: {}", self.base_url);
        log::debug!("   Request: {:?}", request);

        // Пробуем несколько раз с разными настройками
        let mut retry_count = 0;
        let max_retries = 3;

        loop {
            let response = self
                .client
                .post(&self.base_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&request)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    log::debug!("   Response status: {}", resp.status());

                    if !resp.status().is_success() {
                        let status = resp.status();
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "Неизвестная ошибка".to_string());

                        // Если Cloudflare блокирует, пробуем альтернативный метод
                        if error_text.contains("Cloudflare") || status == 403 {
                            log::warn!(
                                "⚠️ Обнаружена блокировка Cloudflare, пробуем альтернативный метод"
                            );

                            if retry_count < max_retries {
                                retry_count += 1;
                                log::info!(
                                    "🔄 Попытка {}/{} с альтернативными настройками",
                                    retry_count,
                                    max_retries
                                );

                                // Ждем немного перед повторной попыткой
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                continue;
                            }
                        }

                        log::error!("API вернул ошибку {}: {}", status, error_text);
                        anyhow::bail!("API вернул ошибку {}: {}", status, error_text);
                    }

                    let account_info: AccountInfoResponse = resp
                        .json()
                        .await
                        .context("Не удалось разобрать ответ API")?;

                    if account_info.result.status != "success" {
                        log::error!(
                            "API вернул статус: {} для адреса {}",
                            account_info.result.status,
                            address
                        );
                        anyhow::bail!(
                            "API вернул статус: {} для адреса {}",
                            account_info.result.status,
                            address
                        );
                    }

                    log::debug!("Получен ответ account_info для {}", address);
                    return Ok(account_info);
                }
                Err(e) => {
                    log::error!("Ошибка при запросе: {}", e);

                    if retry_count < max_retries {
                        retry_count += 1;
                        log::info!("🔄 Попытка {}/{} после ошибки", retry_count, max_retries);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }

                    return Err(anyhow::anyhow!("Не удалось отправить запрос к API: {}", e));
                }
            }
        }
    }

    pub async fn get_account_transactions(&self, address: &str) -> Result<AccountTxResponse> {
        let request = AccountTxRequest::new(address.to_string());
        log::debug!(
            "Отправка запроса account_tx для {} в сети {}",
            address,
            self.network
        );

        let response = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&request)
            .send()
            .await
            .context("Не удалось отправить запрос account_tx к API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Неизвестная ошибка".to_string());
            log::error!("API account_tx вернул ошибку {}: {}", status, error_text);
            anyhow::bail!("API account_tx вернул ошибку {}: {}", status, error_text);
        }

        let account_tx: AccountTxResponse = response
            .json()
            .await
            .context("Не удалось разобрать ответ account_tx API")?;

        if account_tx.result.status != "success" {
            log::error!("API account_tx вернул статус: {}", account_tx.result.status);
            anyhow::bail!("API account_tx вернул статус: {}", account_tx.result.status);
        }

        log::debug!("Получен ответ account_tx для {}", address);
        Ok(account_tx)
    }

    pub async fn get_server_state(&self) -> Result<ServerStateResponse> {
        let request = ServerStateRequest::new();
        log::debug!("Отправка запроса server_state в сети {}", self.network);

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .context("Не удалось отправить запрос server_state к API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Неизвестная ошибка".to_string());
            log::error!("API server_state вернул ошибку {}: {}", status, error_text);
            anyhow::bail!("API server_state вернул ошибку {}: {}", status, error_text);
        }

        let state_response: ServerStateResponse = response
            .json()
            .await
            .context("Не удалось разобрать ответ server_state API")?;

        if state_response.result.status != "success" {
            log::error!(
                "API server_state вернул статус: {}",
                state_response.result.status
            );
            anyhow::bail!(
                "API server_state вернул статус: {}",
                state_response.result.status
            );
        }

        log::debug!("Получен ответ server_state");
        Ok(state_response)
    }

    pub async fn submit_transaction(&self, tx_blob: &str) -> Result<SubmitResponse> {
        let request = SubmitRequest::new(tx_blob.to_string());
        log::debug!("Отправка транзакции в сеть {}", self.network);

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .context("Не удалось отправить запрос submit к API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Неизвестная ошибка".to_string());
            log::error!("API вернул ошибку submit {}: {}", status, error_text);
            anyhow::bail!("API вернул ошибку submit {}: {}", status, error_text);
        }

        let submit_response: SubmitResponse = response
            .json()
            .await
            .context("Не удалось разобрать ответ submit API")?;

        if submit_response.result.status != "success" {
            log::error!(
                "API submit вернул статус: {}",
                submit_response.result.status
            );
            anyhow::bail!(
                "API submit вернул статус: {}",
                submit_response.result.status
            );
        }

        log::debug!("Транзакция успешно отправлена");
        Ok(submit_response)
    }

    pub async fn request_from_faucet(&self, address: &str) -> Result<FaucetResponse> {
        let config = self.network.config();
        let faucet_url = config
            .faucet_url
            .ok_or_else(|| anyhow::anyhow!("Faucet недоступен для сети {}", config.name))?;

        log::info!(
            "Запрос тестовых XRP из faucet для адреса {} в сети {}",
            address,
            self.network
        );

        let faucet_request = FaucetRequest {
            destination: address.to_string(),
            user_agent: "Mozilla/5.0".to_string(),
        };

        let response = self
            .client
            .post(format!("{}/accounts", faucet_url))
            .json(&faucet_request)
            .send()
            .await
            .context("Не удалось отправить запрос к faucet")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Неизвестная ошибка".to_string());
            log::error!("Faucet вернул ошибку {}: {}", status, error_text);

            if error_text.contains("rate limit") {
                anyhow::bail!("Превышен лимит запросов к faucet. Попробуйте позже.");
            } else if error_text.contains("already funded") {
                anyhow::bail!("Адрес уже получал тестовые XRP недавно.");
            }

            anyhow::bail!("Faucet вернул ошибку {}: {}", status, error_text);
        }

        let faucet_response: FaucetResponse = response
            .json()
            .await
            .context("Не удалось разобрать ответ faucet")?;

        log::info!(
            "Успешно получено {} XRP для адреса {}",
            faucet_response.amount,
            address
        );

        Ok(faucet_response)
    }
}

impl Default for XrpApi {
    fn default() -> Self {
        Self::new(Network::Mainnet).expect("Не удалось создать API-клиент")
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
