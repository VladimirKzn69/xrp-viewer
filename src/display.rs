use crate::models::{
    AccountInfoResponse, AccountTxResponse, DisplayAccountInfo, DisplayTransaction, FaucetResponse,
    SubmitResponse,
};
use crate::network::Network;

pub struct DisplayFormatter;

impl DisplayFormatter {
    /// Отображает информацию об аккаунте
    pub fn account_info(response: &AccountInfoResponse, network: &Network) {
        let account_data = &response.result.account_data;
        let display_info =
            DisplayAccountInfo::from_account_data(account_data.account.clone(), account_data);

        println!("\n╔══════════════════════════════════════════╗");
        println!("║           ИНФОРМАЦИЯ О КОШЕЛЬКЕ          ║");
        println!("╠══════════════════════════════════════════╣");
        println!("║ 🌐 Сеть:     {:28} ║", network.to_string());
        println!(
            "║ 📍 Адрес:    {:28} ║",
            truncate_address(&display_info.address)
        );
        println!("║ 💰 Баланс:   {:>20.6} XRP    ║", display_info.balance_xrp);
        println!("║ 🔢 Sequence: {:>28} ║", account_data.sequence);
        println!("╚══════════════════════════════════════════╝");

        // Предупреждение для тестовых сетей
        if !network.config().is_production {
            println!("\n⚠️  Это тестовая сеть! Транзакции не имеют реальной стоимости.");
        }
    }

    /// Отображает список транзакций
    pub fn transactions(response: &AccountTxResponse) {
        let transactions: Vec<DisplayTransaction> = response
            .result
            .transactions
            .iter()
            .filter_map(|tx_wrapper| DisplayTransaction::from_transaction(&tx_wrapper.tx))
            .take(5)
            .collect();

        if transactions.is_empty() {
            println!("\n📭 Транзакций не найдено");
            return;
        }

        println!("\n📜 Последние транзакции:");
        println!("┌────────────────────┬──────────────────┬────────────────┬────────────────┐");
        println!("│       Дата         │   Сумма/Токен    │       От       │       Кому     │");
        println!("├────────────────────┼──────────────────┼────────────────┼────────────────┤");

        for tx in transactions {
            // Определяем что показывать - XRP или токены
            let amount_str = if let Some(token_info) = &tx.token_info {
                // Это транзакция с токеном
                format!("{:>16}", token_info)
            } else if tx.amount_xrp > 0.0 {
                // Это XRP транзакция
                format!("{:>14.2} XRP", tx.amount_xrp)
            } else {
                // Неизвестный тип
                format!("{:>16}", "---")
            };

            println!(
                "│ {:18} │ {:16} │ {:14} │ {:14} │",
                truncate_string(&tx.timestamp, 18),
                amount_str,
                truncate_address(&tx.from),
                truncate_address(&tx.to)
            );
        }
        println!("└────────────────────┴──────────────────┴────────────────┴────────────────┘");

        // Подсказка о токенах
        let has_tokens = response
            .result
            .transactions
            .iter()
            .any(|tx| tx.tx.is_token_transaction());

        if has_tokens {
            println!("\n💡 Подсказка: Этот адрес имеет транзакции с токенами (не XRP)");
        }
    }

    /// Отображает результат отправки транзакции
    pub fn submit_result(response: &SubmitResponse, network: &Network) {
        let result = &response.result;

        if result.engine_result == "tesSUCCESS" {
            println!("\n✅ Транзакция отправлена успешно!");
            println!("   Сеть: {}", network);
            println!("   Результат: {}", result.engine_result);
            println!("   Сообщение: {}", result.engine_result_message);

            // Извлекаем хэш транзакции из tx_json
            if let Some(hash) = result.tx_json.get("hash").and_then(|h| h.as_str()) {
                println!("   Хэш: {}", hash);

                // Показываем ссылку на эксплорер
                let config = network.config();
                println!("\n🔗 Посмотреть в эксплорере:");
                println!("   {}", config.transaction_url(hash));
            }
        } else {
            println!("\n❌ Ошибка отправки транзакции");
            println!("   Сеть: {}", network);
            println!("   Код: {}", result.engine_result);
            println!("   Сообщение: {}", result.engine_result_message);

            // Дополнительные подсказки по ошибкам
            match result.engine_result.as_str() {
                "tecUNFUNDED_PAYMENT" => {
                    println!("\n💡 Недостаточно средств для отправки");
                }
                "tecNO_DST" => {
                    println!("\n💡 Адрес получателя не активирован (требуется минимум 10 XRP)");
                }
                "tefBAD_AUTH" => {
                    println!("\n💡 Неверная подпись транзакции");
                }
                _ => {}
            }
        }
    }

    /// Отображает результат запроса к faucet
    pub fn faucet_result(response: &FaucetResponse, network: &Network) {
        println!("\n✅ Тестовые XRP успешно получены!");
        println!("   Сеть: {}", network);
        println!("   Адрес: {}", response.account);
        println!("   Сумма: {} drops", response.amount);

        // Конвертируем drops в XRP
        if let Ok(drops) = response.amount.parse::<u64>() {
            let xrp = drops as f64 / 1_000_000.0;
            println!("   В XRP: {:.6} XRP", xrp);
        }

        if let Some(balance) = &response.balance {
            println!("   Новый баланс: {} drops", balance);
        }

        if let Some(tx_hash) = &response.tx_hash {
            println!("   Транзакция: {}", tx_hash);

            // Показываем ссылку на эксплорер
            let config = network.config();
            println!("\n🔗 Посмотреть в эксплорере:");
            println!("   {}", config.transaction_url(tx_hash));
        }
    }

    /// Отображает информацию о сети
    pub fn network_info(network: &Network) {
        let config = network.config();

        println!("\n╔══════════════════════════════════════════════════════╗");
        println!("║                 ИНФОРМАЦИЯ О СЕТИ                    ║");
        println!("╠══════════════════════════════════════════════════════╣");
        println!("║ 🌐 Название: {:40} ║", config.name);
        println!(
            "║ 🔗 RPC URL:  {:40} ║",
            truncate_string(config.rpc_url, 40)
        );
        println!(
            "║ 🔍 Explorer: {:40} ║",
            truncate_string(config.explorer_url, 40)
        );

        if let Some(ws_url) = config.ws_url {
            println!("║ 🔌 WebSocket: {:39} ║", truncate_string(ws_url, 39));
        }

        if let Some(faucet_url) = config.faucet_url {
            println!("║ 💧 Faucet:   {:40} ║", truncate_string(faucet_url, 40));
        }

        println!(
            "║ 🏷️  Тип:      {:40} ║",
            if config.is_production {
                "Production (Реальная сеть)"
            } else {
                "Testnet (Тестовая сеть)"
            }
        );
        println!("╚══════════════════════════════════════════════════════╝");

        if !config.is_production {
            println!("\n⚠️  Это тестовая сеть!");
            println!("   • Транзакции не имеют реальной стоимости");
            println!("   • Можно получить бесплатные XRP через faucet");
            println!("   • Идеально для тестирования и разработки");
        } else {
            println!("\n⚡ Это основная сеть (mainnet)!");
            println!("   • Все транзакции реальные");
            println!("   • Используйте с осторожностью");
        }

        // Показываем команды для текущей сети
        println!("\n📝 Примеры команд для сети {}:", network);
        println!("   cargo run -- -n {} balance <адрес>", network);

        if !config.is_production {
            println!("   cargo run -- -n {} faucet <адрес>", network);
        }

        println!(
            "   cargo run -- -n {} send --from <адрес> --to <адрес> --amount <сумма>",
            network
        );
    }

    /// Отображает ошибку
    pub fn error(message: &str) {
        println!("\n❌ Ошибка: {}", message);
    }

    /// Отображает предупреждение
    pub fn warning(message: &str) {
        println!("\n⚠️  Предупреждение: {}", message);
    }

    /// Отображает информационное сообщение
    pub fn info(message: &str) {
        println!("\nℹ️  {}", message);
    }
}

// Вспомогательные функции
fn truncate_address(address: &str) -> String {
    if address.len() <= 14 {
        address.to_string()
    } else {
        format!("{}...{}", &address[..6], &address[address.len() - 4..])
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
