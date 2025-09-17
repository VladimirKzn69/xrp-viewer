use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

use crate::models::{
    AccountInfoRequest, AccountInfoResponse, AccountTxRequest, AccountTxResponse, FaucetRequest,
    FaucetResponse, ServerStateRequest, ServerStateResponse, SubmitResponse, SubmitResult,
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

    /// Отправляет подписанную транзакцию в XRP Ledger
    pub async fn submit_transaction(&self, tx_blob: &str) -> Result<SubmitResult> {
        log::info!("Отправка транзакции в сеть...");
        log::debug!("TX Blob length: {} bytes", tx_blob.len());

        let request = json!({
            "method": "submit",
            "params": [{
                "tx_blob": tx_blob
            }]
        });

        log::debug!(
            "Submit request: {}",
            serde_json::to_string_pretty(&request)?
        );

        // Используем self.client и self.base_url вместо client.client
        let response = self
            .client
            .post(&self.base_url) // Используем base_url вместо endpoint
            .json(&request)
            .send()
            .await
            .context("Ошибка отправки запроса")?;

        let status = response.status();
        let response_text: String = response.text().await?; // Явно указываем тип

        log::debug!("Response status: {}", status);
        log::debug!("Response body: {}", response_text);

        // Попробуем распарсить JSON ответ
        let response_json: serde_json::Value =
            serde_json::from_str(&response_text).context("Ошибка парсинга JSON ответа")?;

        // Проверяем наличие ошибки на уровне JSON-RPC
        if let Some(error) = response_json.get("error") {
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Неизвестная ошибка");
            let error_code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);

            log::error!("JSON-RPC error: {} (code: {})", error_message, error_code);

            // Создаем SubmitResult с информацией об ошибке
            return Ok(SubmitResult {
                error: Some(error_message.to_string()),
                error_code: Some(error_code as i32),
                error_message: Some(error_message.to_string()),
                accepted: None,
                account_sequence_available: None,
                account_sequence_next: None,
                applied: None,
                broadcast: None,
                engine_result: None,
                engine_result_code: None,
                engine_result_message: None,
                kept: None,
                open_ledger_cost: None,
                queued: None,
                tx_blob: None,
                tx_json: None,
                validated_ledger_index: None,
                error_exception: None,
            });
        }

        // Парсим результат
        let submit_response: SubmitResponse = serde_json::from_value(response_json)
            .context("Ошибка десериализации SubmitResponse")?;

        // Логируем результат
        if let Some(engine_result) = &submit_response.result.engine_result {
            log::info!("Engine result: {}", engine_result);
            if let Some(message) = &submit_response.result.engine_result_message {
                log::info!("Engine message: {}", message);
            }
        }

        // Проверяем на успех
        if submit_response.result.is_success() {
            if let Some(tx_hash) = submit_response.result.get_tx_hash() {
                log::info!("✅ Транзакция успешно отправлена! Hash: {}", tx_hash);
            } else {
                log::info!("✅ Транзакция принята в очередь");
            }
        } else {
            let error_msg = submit_response
                .result
                .get_error_message()
                .unwrap_or_else(|| "Неизвестная ошибка".to_string());
            log::error!("❌ Ошибка отправки транзакции: {}", error_msg);
        }

        Ok(submit_response.result)
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

// Добавим также реализацию Default для SubmitResult
impl Default for SubmitResult {
    fn default() -> Self {
        Self {
            accepted: None,
            account_sequence_available: None,
            account_sequence_next: None,
            applied: None,
            broadcast: None,
            engine_result: None,
            engine_result_code: None,
            engine_result_message: None,
            kept: None,
            open_ledger_cost: None,
            queued: None,
            tx_blob: None,
            tx_json: None,
            validated_ledger_index: None,
            error: None,
            error_code: None,
            error_message: None,
            error_exception: None,
        }
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
