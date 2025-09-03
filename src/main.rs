// main.rs - точка входа в программу с поддержкой отправки XRP
use anyhow::{Context, Result};
use clap::Parser;

// Подключаем наши модули
mod api;
mod config;
mod crypto;
mod display;
mod models;
mod network;
mod xrp_codec; // 🆕 Новый модуль для XRP Binary Codec

// Подключаем конкретные элементы из модулей
use api::XrpApi;
use config::{get_private_key, load_env_file};
use crypto::{
    canonical_serialize, create_signed_tx_blob, decode_wif, derive_public_key,
    is_valid_xrp_address, sign_blob,
};
use display::DisplayFormatter;
use models::{PaymentFields, TransactionCommonFields};

/// XRP кошелек: просмотр баланса и отправка транзакций
#[derive(Debug, Parser)]
#[clap(name = "xrp-viewer", version = "0.2.0", about = "XRP кошелек")]
enum Cli {
    /// Просмотр баланса и последней транзакции кошелька
    Balance {
        /// Публичный XRP-адрес (начинается с 'r')
        address: String,
    },
    /// Отправить XRP
    Send {
        /// Адрес отправителя
        #[clap(long)]
        from: String,
        /// Адрес получателя
        #[clap(long)]
        to: String,
        /// Сумма в XRP
        #[clap(long)]
        amount: f64,
        /// Путь к файлу с приватным ключом (.env)
        #[clap(long, default_value = ".env")]
        key_file: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Инициализация логирования
    env_logger::init();

    // Парсинг аргументов командной строки
    let cli = Cli::parse();

    match cli {
        Cli::Balance { address } => {
            handle_balance(address).await?;
        }
        Cli::Send {
            from,
            to,
            amount,
            key_file,
        } => {
            handle_send(from, to, amount, key_file).await?;
        }
    }

    Ok(())
}

/// Обработка команды balance
async fn handle_balance(address: String) -> Result<()> {
    log::debug!("Получен адрес для баланса: {}", address);

    // Проверяем валидность адреса

    if !is_valid_xrp_address(&address) {
        eprintln!("❌ Ошибка: Неверный формат XRP адреса");
        eprintln!("💡 XRP адрес должен начинаться с 'r' и быть валидным Base58");
        return Err(anyhow::anyhow!("Неверный формат адреса"));
    }

    // Создаём клиент API
    let api_client = XrpApi::new().context("Не удалось создать клиент API")?;

    // Получаем информацию о кошельке
    println!("🔄 Подключение к XRP Ledger...");

    match api_client.get_account_info(&address).await {
        Ok(account_info) => {
            log::info!("Получена информация о кошельке");

            // Получаем последнюю транзакцию
            let transaction = api_client
                .get_latest_transaction(&address)
                .await
                .ok()
                .flatten();

            // Создаём форматировщик вывода
            let formatter = DisplayFormatter::new();

            // Выводим информацию
            formatter.display_account_info(
                &address,
                &account_info.result.account_data,
                transaction.as_ref(),
            );

            Ok(())
        }
        Err(e) => {
            log::error!("Ошибка API: {}", e);

            if e.to_string().contains("actNotFound") || e.to_string().contains("Account not found")
            {
                eprintln!("❌ Ошибка: Кошелек не найден или не активирован");
                eprintln!("💡 Для активации кошелька необходимо минимум 1 XRP");
            } else {
                eprintln!("❌ Ошибка: Не удается подключиться к API");
                eprintln!("💡 Проверьте интернет-соединение");
            }

            Err(anyhow::anyhow!("Ошибка получения информации о кошельке"))
        }
    }
}

/// Обработка команды send
async fn handle_send(from: String, to: String, amount: f64, key_file: String) -> Result<()> {
    log::info!("🚀 Начинаем процесс отправки XRP");
    log::debug!(
        "From: {}, To: {}, Amount: {}, Key File: {}",
        from,
        to,
        amount,
        key_file
    );

    // 1. Валидация адресов
    println!("🔍 Проверка адресов...");

    if !is_valid_xrp_address(&from) {
        eprintln!("❌ Ошибка: Неверный адрес отправителя");
        return Err(anyhow::anyhow!("Неверный адрес отправителя"));
    }

    if !is_valid_xrp_address(&to) {
        eprintln!("❌ Ошибка: Неверный адрес получателя");
        return Err(anyhow::anyhow!("Неверный адрес получателя"));
    }

    if amount <= 0.0 {
        eprintln!("❌ Ошибка: Сумма должна быть больше 0");
        return Err(anyhow::anyhow!("Неверная сумма"));
    }

    println!("✅ Адреса валидны");

    // 2. Загрузка приватного ключа
    println!("🔑 Загрузка приватного ключа...");

    load_env_file(&key_file)
        .with_context(|| format!("Не удалось загрузить файл '{}'", key_file))?;

    let private_key_wif =
        get_private_key().context("Не удалось получить приватный ключ из конфигурации")?;

    println!("✅ Ключ загружен");

    // 3. Декодирование ключа
    let private_key_bytes =
        decode_wif(&private_key_wif).context("Не удалось декодировать приватный ключ (WIF)")?;

    let public_key_bytes =
        derive_public_key(&private_key_bytes).context("Не удалось получить публичный ключ")?;

    log::debug!("Публичный ключ: {} байт", public_key_bytes.len());

    // 4. Создание клиента API
    let api = XrpApi::new().context("Не удалось создать клиент API")?;

    // 5. Получение информации об аккаунте
    println!("📊 Получение информации о балансе...");

    let account_info = api
        .get_account_info(&from)
        .await
        .context("Не удалось получить информацию об аккаунте отправителя")?;

    let balance_drops = account_info
        .result
        .account_data
        .balance
        .parse::<u64>()
        .unwrap_or(0);
    let balance_xrp = balance_drops as f64 / 1_000_000.0;

    let sequence = account_info.result.account_data.sequence.unwrap_or(0);

    println!("💰 Текущий баланс: {:.6} XRP", balance_xrp);
    println!("📝 Sequence: {}", sequence);

    // Проверка достаточности баланса
    let amount_drops = (amount * 1_000_000.0) as u64;
    let min_reserve = 10_000_00; // 1 XRP минимальный резерв
    let fee_drops = 12; // Базовая комиссия

    if balance_drops < amount_drops + fee_drops + min_reserve {
        eprintln!("❌ Недостаточно средств!");
        eprintln!("   Баланс: {:.6} XRP", balance_xrp);
        eprintln!(
            "   Необходимо: {:.6} XRP (включая резерв и комиссию)",
            (amount_drops + fee_drops + min_reserve) as f64 / 1_000_000.0
        );
        return Err(anyhow::anyhow!("Недостаточно средств"));
    }

    // 6. Получение состояния сервера для LastLedgerSequence
    println!("🌐 Получение состояния сети...");

    let server_state = api
        .get_server_state()
        .await
        .context("Не удалось получить состояние сервера")?;

    let current_ledger = server_state
        .result
        .state
        .validated_ledger
        .ledger_index
        .unwrap_or(0);
    let last_ledger_sequence = current_ledger + 10; // Транзакция действительна 10 ledger'ов

    // Получаем рекомендуемую комиссию
    let default_fee = "0.000012".to_string(); // Создаём долгоживущую переменную
    let base_fee_str = server_state
        .result
        .state
        .validated_ledger
        .base_fee_xrp
        .as_ref()
        .unwrap_or(&default_fee); // Ссылаемся на неё
    let recommended_fee = (base_fee_str.parse::<f64>().unwrap_or(0.000012) * 1_000_000.0) as u64;
    let fee_drops_final = recommended_fee.max(12); // Минимум 12 drops

    println!("✅ Текущий ledger: {}", current_ledger);
    println!(
        "💸 Рекомендуемая комиссия: {} drops ({:.6} XRP)",
        fee_drops_final,
        fee_drops_final as f64 / 1_000_000.0
    );

    // 7. Подтверждение от пользователя
    println!("\n📋 Детали транзакции:");
    println!("┌─────────────────────────────────────");
    println!("│ От: {}", from);
    println!("│ Кому: {}", to);
    println!("│ Сумма: {:.6} XRP", amount);
    println!(
        "│ Комиссия: {:.6} XRP",
        fee_drops_final as f64 / 1_000_000.0
    );
    println!(
        "│ Итого будет списано: {:.6} XRP",
        (amount_drops + fee_drops_final) as f64 / 1_000_000.0
    );
    println!("└─────────────────────────────────────");

    println!("\n❓ Отправить транзакцию? (y/N): ");

    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("❌ Отменено пользователем");
        return Ok(());
    }

    // 8. Создание и подписание транзакции
    println!("🔐 Подписание транзакции...");

    let common_fields = TransactionCommonFields {
        transaction_type: "Payment".to_string(),
        account: from.clone(),
        fee: fee_drops_final.to_string(),
        sequence,
        last_ledger_sequence: Some(last_ledger_sequence),
    };

    let payment_fields = PaymentFields {
        amount: amount_drops.to_string(),
        destination: to.clone(),
    };

    // Сериализация для подписи
    let transaction_blob = canonical_serialize(&common_fields, &payment_fields)
        .context("Не удалось сериализовать транзакцию")?;

    log::debug!("Транзакция сериализована: {} байт", transaction_blob.len());

    // Подписание
    let signature_der = sign_blob(&private_key_bytes, &transaction_blob)
        .context("Не удалось подписать транзакцию")?;

    log::debug!("Подпись создана: {} байт", signature_der.len());

    // Создание финального blob
    let tx_blob = create_signed_tx_blob(
        transaction_blob,
        signature_der,
        public_key_bytes,
        &common_fields,
        &payment_fields,
    )
    .context("Не удалось создать финальный tx_blob")?;

    println!("✅ Транзакция подписана");
    log::debug!("TX Blob длина: {} символов", tx_blob.len());

    // 9. Отправка транзакции
    println!("📡 Отправка транзакции в сеть...");

    match api.submit_transaction(&tx_blob).await {
        Ok(submit_result) => {
            let engine_result = &submit_result.result.engine_result;
            let engine_code = submit_result.result.engine_result_code;
            let engine_message = &submit_result.result.engine_result_message;

            println!("\n📬 Результат отправки:");
            println!("   Статус: {} (код: {})", engine_result, engine_code);
            println!("   Сообщение: {}", engine_message);

            if engine_code == 0 || engine_result.starts_with("tes") {
                println!("\n✅ Транзакция успешно отправлена!");

                if let Some(hash) = submit_result.result.get_transaction_hash() {
                    println!("🔗 Хэш транзакции: {}", hash);
                    println!("\n💡 Отслеживать транзакцию можно на:");
                    println!("   https://livenet.xrpl.org/transactions/{}", hash);
                }
            } else if engine_result.starts_with("tec") {
                println!("\n⚠️ Транзакция обработана, но с ошибкой!");
                println!("   Проверьте детали ошибки выше");
            } else {
                println!("\n❌ Транзакция отклонена!");

                // Подробные подсказки по частым ошибкам
                match engine_result.as_str() {
                    "temMALFORMED" => {
                        println!("   💡 Неверный формат транзакции");
                    }
                    "tefBAD_AUTH" => {
                        println!("   💡 Неверная подпись или ключ не соответствует адресу");
                    }
                    "tecUNFUNDED_PAYMENT" => {
                        println!("   💡 Недостаточно средств для отправки");
                    }
                    "tecNO_DST" => {
                        println!("   💡 Адрес получателя не существует или не активирован");
                    }
                    _ => {
                        println!("   💡 См. документацию XRP для кода: {}", engine_result);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("\n❌ Ошибка при отправке транзакции: {}", e);

            if e.to_string().contains("timeout") {
                eprintln!("   💡 Превышено время ожидания. Проверьте интернет-соединение");
            } else {
                eprintln!("   💡 Проверьте правильность данных и повторите попытку");
            }

            return Err(e);
        }
    }

    Ok(())
}
