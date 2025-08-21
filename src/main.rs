// main.rs - точка входа в программу
use clap::Parser;
// use std::process; // Убираем, так как используем anyhow и возвращаем ошибку из main
// Подключаем наши модули
mod api;
mod display;
mod models;
mod config;
mod crypto; // Подключаем новый модуль

// Подключаем конкретные элементы из модулей
use api::XrpApi;
use display::DisplayFormatter;
// Импортируем нужные функции из config
use config::{load_env_file, get_private_key};
// Импортируем нужные функции из crypto
use crypto::{
    decode_wif, derive_public_key, sign_blob, canonical_serialize_stub, create_signed_tx_blob_stub,
};

// Для удобной обработки ошибок
use anyhow::{Context, Result};
use serde_json::json;
use crate::models::{TransactionCommonFields, PaymentFields, SubmitResult}; // Добавляем нужные модели

/// XRP кошелек: просмотр баланса и отправка транзакций.
#[derive(Debug, Parser)]
#[clap(name = "xrp-viewer", version = "0.2.0", about = "XRP кошелек")]
enum Cli {
    /// Просмотр баланса и последней транзакции кошелька
    #[clap(name = "balance")]
    Balance {
        /// Публичный XRP-адрес (начинается с 'r')
        address: String,
    },
    /// Отправка XRP с одного кошелька на другой
    #[clap(name = "send")]
    Send {
        /// Адрес отправителя (публичный ключ)
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
// Меняем тип ошибки на anyhow::Result
async fn main() -> Result<()> {
    // Инициализация логирования
    env_logger::init();
    // Парсинг аргументов командной строки
    let cli = Cli::parse();
    match cli {
        Cli::Balance { address } => {
            // handle_balance теперь тоже возвращает anyhow::Result
            handle_balance(address).await?;
        }
        Cli::Send {
            from,
            to,
            amount,
            key_file,
        } => {
            // handle_send теперь тоже возвращает anyhow::Result
            handle_send(from, to, amount, key_file).await?;
        }
    }
    Ok(()) // Успешное завершение программы
}

/// Обрабатывает подкоманду 'balance'
// Меняем тип ошибки на anyhow::Result
async fn handle_balance(address: String) -> Result<()> {
    log::debug!("Получен адрес для баланса: {}", address);
    // Создаем клиент API
    // Используем .context для лучшего сообщения об ошибке
    let api_client = XrpApi::new().context("Не удалось создать клиент API")?;
    // Получаем информацию о кошельке
    match api_client.get_account_info(&address).await {
        Ok(account_info) => {
            log::info!("Получена информация о кошельке");
            // Получаем последние транзакции
            match api_client.get_latest_transaction(&address).await {
                Ok(transaction) => {
                    // Создаем форматировщик вывода
                    let formatter = DisplayFormatter::new();
                    // Выводим информацию
                    formatter.display_account_info(
                        &address,
                        &account_info.result.account_data,
                        transaction.as_ref(),
                    );
                    Ok(()) // Успех
                }
                Err(e) => {
                    log::error!("Ошибка получения транзакций: {}", e);
                    // Используем eprintln для вывода ошибки пользователю
                    eprintln!("Ошибка: Не удается подключиться к API");
                    // Возвращаем ошибку с помощью anyhow
                    Err(anyhow::anyhow!("Ошибка получения транзакций"))
                }
            }
        }
        Err(e) => {
            log::error!("Ошибка API: {}", e);
            if e.to_string().contains("actNotFound")
                || e.to_string().contains("Account not found")
            {
                eprintln!("Ошибка: Кошелек не найден");
            } else {
                eprintln!("Ошибка: Не удается подключиться к API");
            }
            // Возвращаем ошибку с помощью anyhow
            Err(anyhow::anyhow!("Ошибка получения информации о кошельке"))
        }
    }
}

/// Обрабатывает подкоманду 'send'
// Меняем тип ошибки на anyhow::Result
async fn handle_send(
    from: String,
    to: String,
    amount: f64,
    key_file: String,
) -> Result<()> {
    log::info!("📤 Начинаем процесс отправки...");
    log::debug!("From: {}, To: {}, Amount: {}, Key File: {}", from, to, amount, key_file);
    // 1. Загрузка .env файла
    load_env_file(&key_file)
        .with_context(|| format!("Не удалось загрузить файл конфигурации '{}'", key_file))?;
    // 2. Получение приватного ключа
    let private_key_wif = get_private_key() // WIF = Wallet Import Format
        .context("Не удалось получить приватный ключ из конфигурации")?;
    // 3. Вывод информации (временно, для проверки)
    log::info!("✅ .env файл '{}' успешно загружен.", key_file);
    // ВАЖНО: Никогда не выводите приватный ключ в логи или на экран!
    // Для отладки показываем только часть ключа
    if private_key_wif.len() > 6 {
        println!("🔑 Приватный ключ (WIF) загружен (первые 6 символов): {}...", &private_key_wif[..6]);
    } else {
         println!("🔑 Приватный ключ (WIF) загружен (ключ короткий).");
    }
    println!("📤 Подготовка к отправке {} XRP с {} на {}", amount, from, to);
    // 4. Создание клиента API
    let api_client = XrpApi::new().context("Не удалось создать клиент API")?;
    log::debug!("Клиент API создан.");
    // 5. Получение информации об аккаунте отправителя (для sequence)
    let account_info = api_client.get_account_info(&from)
        .await
        .context("Не удалось получить информацию об аккаунте отправителя")?;
    let sequence = account_info.result.account_data.sequence.unwrap_or(0);
    log::debug!("Sequence для {}: {}", from, sequence);
    // 5.1. Получение текущей комиссии (fee) и ledger index с сервера
    let server_state = api_client.get_server_state() // Предполагаем, что метод get_server_state уже добавлен в api.rs
        .await
        .context("Не удалось получить состояние сервера для определения комиссии")?;
    // Исправленный код:
    let default_fee = "0.000010".to_string(); // Создаем "долгоживущую" строку
    let base_fee_xrp_str = server_state.result.state.validated_ledger.base_fee_xrp.as_ref()
    .unwrap_or(&default_fee); // Ссылаемся на неё
    let fee_drops = (base_fee_xrp_str.parse::<f64>().unwrap_or(0.000010) * 1_000_000.0) as u64;
    log::debug!("Рекомендуемая комиссия (XRP): {}, (drops): {}", base_fee_xrp_str, fee_drops);
    let current_ledger_index = server_state.result.state.validated_ledger.ledger_index.unwrap_or(1);
    let last_ledger_sequence = current_ledger_index + 10; // Транзакция действительна 10 леджеров
    log::debug!("Текущий ledger index: {}, LastLedgerSequence: {}", current_ledger_index, last_ledger_sequence);
    // 6. Создание транзакции (в памяти, структура данных)
    let amount_drops = (amount * 1_000_000.0) as u64;
    let common_fields = TransactionCommonFields {
        transaction_type: "Payment".to_string(),
        account: from.clone(),
        fee: fee_drops.to_string(),
        sequence,
        last_ledger_sequence: Some(last_ledger_sequence),
    };
    let payment_fields = PaymentFields {
        amount: amount_drops.to_string(),
        destination: to.clone(),
    };
    log::debug!("Созданы структуры common_fields: {:?} и payment_fields: {:?}", common_fields, payment_fields);
    // --- Основная логика подписания ---
    // Шаг 1: Декодируем WIF приватный ключ
    let private_key_bytes = decode_wif(&private_key_wif)
        .context("Не удалось декодировать приватный ключ (WIF)")?;
    log::debug!("Приватный ключ (32 байта) успешно декодирован.");
    // Шаг 2: Получаем публичный ключ
    let public_key_bytes = derive_public_key(&private_key_bytes)
        .context("Не удалось получить публичный ключ")?;
    let public_key_hex = hex::encode(&public_key_bytes);
    log::debug!("Публичный ключ (hex, uncompressed): {}", public_key_hex);
    // Шаг 3: Каноничная сериализация транзакции (ЗАГЛУШКА)
    let transaction_blob = canonical_serialize_stub(&common_fields, &payment_fields)
        .context("Не удалось сериализовать транзакцию")?;
    log::debug!("Транзакция сериализована в blob ({} байт)", transaction_blob.len());
    // Шаг 4: Подписываем blob
    let signature_der = sign_blob(&private_key_bytes, &transaction_blob)
        .context("Не удалось подписать транзакцию")?;
    let signature_hex = hex::encode(&signature_der);
    log::debug!("Транзакция подписана. Подпись (DER, hex): {}", signature_hex);
    // Шаг 5: Создаём финальный tx_blob (ЗАГЛУШКА)
    let tx_blob = create_signed_tx_blob_stub(transaction_blob, signature_der, public_key_bytes)
        .context("Не удалось создать финальный tx_blob")?;
    log::debug!("Финальный tx_blob создан ({} символов)", tx_blob.len());
    // ---------------------------------------------------------
    // 7. Отправка транзакции
    println!("📡 Отправка транзакции в сеть XRP...");
    let submit_result = api_client.submit_transaction(&tx_blob).await
        .context("Не удалось отправить транзакцию")?;
    // 8. Обработка результата
    let engine_result = &submit_result.result.engine_result;
    let engine_result_code = submit_result.result.engine_result_code;
    let engine_result_message = &submit_result.result.engine_result_message;
    // Используем наш новый метод
    let transaction_hash = submit_result.result.get_transaction_hash()
        .unwrap_or_else(|| "Неизвестен".to_string());
    println!("📬 Результат отправки транзакции:");
    println!("   Engine Result: {} ({})", engine_result, engine_result_code);
    println!("   Message: {}", engine_result_message);
    if engine_result_code == 0 || engine_result.starts_with("tes") {
        println!("✅ Транзакция принята в сеть!");
        println!("   Хэш транзакции: {}", transaction_hash);
    } else {
        println!("❌ Транзакция не принята. Проверьте ошибку выше.");
        // Возвращаем ошибку через anyhow
        anyhow::bail!("Транзакция не принята: {} - {}", engine_result, engine_result_message);
    }
    Ok(()) // Успех
}