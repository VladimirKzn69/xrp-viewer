use crate::config::Config;
use anyhow::anyhow;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::str::FromStr;

use k256::ecdsa::VerifyingKey;
use k256::PublicKey;

// Подключаем наши модули
mod api;
mod config;
mod crypto;
mod display;
mod models;
mod network;
mod xrp_codec;

// Подключаем конкретные элементы из модулей
use crate::crypto::derive_xrp_address_from_public_key;
use api::XrpApi;
use crypto::{
    canonical_serialize, create_signed_tx_blob, decode_private_key, derive_public_key,
    is_valid_xrp_address, sign_blob,
};
use display::DisplayFormatter;
use models::{PaymentFields, TransactionCommonFields};
use network::Network;

/// XRP кошелек: просмотр баланса и отправка транзакций
#[derive(Debug, Parser)]
#[clap(
    name = "xrp-viewer",
    version = "0.3.0",
    about = "XRP кошелек с поддержкой testnet"
)]
struct Cli {
    /// Выбор сети: mainnet, testnet, devnet
    #[clap(long, short = 'n', default_value = "mainnet", global = true)]
    network: String,

    /// Вывод подробной информации
    #[clap(long, short = 'v', global = true)]
    verbose: bool,

    #[clap(subcommand)]
    command: Commands,
    #[clap(long, default_value = ".env")]
    key_file: String,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Просмотр баланса кошелька
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
    /// Получить тестовые XRP из faucet (только testnet/devnet)
    Faucet {
        /// XRP адрес для получения тестовых токенов
        address: String,
    },
    /// Показать информацию о текущей сети
    Network,
    /// Показать адрес, соответствующий ключу из .env
    ShowAddress {
        /// Сеть для проверки
        #[arg(short = 'n', long, default_value = "mainnet")]
        network: Network,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Инициализация логирования
    env_logger::init();

    // Парсинг аргументов командной строки
    let cli = Cli::parse();

    // Парсинг сети
    let network = Network::from_str(&cli.network)
        .context("Неверное название сети. Доступны: mainnet, testnet, devnet")?;

    // Включаем подробное логирование если указан флаг
    if cli.verbose {
        log::info!("Работаем в сети: {}", network);
    }

    let config = Config::load(&cli.key_file, network.clone())?;

    // Замените блок match в main.rs (строки 100-197) на этот исправленный вариант:
    match cli.command {
        Commands::Balance { address } => {
            handle_balance(address, network).await?;
        }
        Commands::Send {
            from,
            to,
            amount,
            key_file,
        } => {
            handle_send(from, to, amount, key_file, network).await?;
        }
        Commands::Network => {
            // Создадим простой вывод информации о сети
            println!("\n🌐 Информация о сети\n");
            println!("Сеть: {}", network.config().name);
            println!("RPC URL: {}", network.config().rpc_url);
            if let Some(faucet) = network.config().faucet_url {
                println!("Faucet URL: {}", faucet);
            }
        }
        Commands::Faucet { address } => {
            handle_faucet(address, network).await?;
        }
        Commands::ShowAddress { network: _ } => {
            use crate::crypto::decode_private_key;

            // config здесь - переменная, а не модуль
            let private_key_str = config
                .private_key
                .ok_or_else(|| anyhow!("Приватный ключ не найден в конфигурации"))?;

            println!("\n🔍 Определение адреса из приватного ключа...\n");

            // Определяем тип ключа и декодируем
            let signing_key = if private_key_str.starts_with('s') {
                println!("📋 Обнаружен XRP Family Seed (начинается с 's')");
                println!(
                    "   Seed: {}...{}",
                    &private_key_str[..4],
                    &private_key_str[private_key_str.len() - 4..]
                );

                // Пока просто возвращаем ошибку, так как модуль seed еще не создан
                return Err(anyhow!(
                    "Поддержка XRP seed пока не реализована. Используйте hex приватный ключ."
                ));
            } else {
                println!("🔑 Обнаружен приватный ключ в hex формате");
                decode_private_key(&private_key_str)
                    .context("Не удалось декодировать приватный ключ")?
            };

            // Получаем публичный ключ
            let public_key = derive_public_key(&signing_key)?;
            // Генерируем адрес
            // Если public_key это Vec<u8>, преобразуем его в VerifyingKey
            let verifying_key = if public_key.len() == 33 {
                // Compressed public key
                let public_key_point = PublicKey::from_sec1_bytes(&public_key)
                    .context("Не удалось декодировать публичный ключ")?;
                VerifyingKey::from(&public_key_point)
            } else {
                return Err(anyhow!("Неверный формат публичного ключа"));
            };

            let address = derive_xrp_address_from_public_key(&verifying_key)?;

            // Выводим результаты
            println!("\n✅ Адрес успешно определен!\n");
            println!("📍 Ваш XRP адрес: {}", address);
            println!("\n💡 Используйте этот адрес для:");
            println!("   • Проверки баланса: cargo run -- balance {}", address);
            println!(
                "   • Отправки XRP: cargo run -- send --from {} --to <адрес> --amount <сумма>",
                address
            );
            println!("\n⚠️  Убедитесь, что адрес имеет достаточный баланс перед отправкой!");
        }
    }

    Ok(())
}

/// Обработка команды balance
async fn handle_balance(address: String, network: Network) -> Result<()> {
    log::debug!(
        "Выполнение команды balance для {} в сети {}",
        address,
        network
    );

    // Валидация адреса
    if !is_valid_xrp_address(&address) {
        anyhow::bail!(
            "Неверный формат XRP адреса: {}. Адрес должен начинаться с 'r'",
            address
        );
    }

    // Создаем API клиент с указанной сетью
    let api = XrpApi::new(network)?;

    // Получаем информацию об аккаунте
    println!("⏳ Получение информации о балансе в сети {}...", network);
    let account_info = api
        .get_account_info(&address)
        .await
        .context("Не удалось получить информацию об аккаунте")?;

    // Получаем последние транзакции
    let transactions = api
        .get_account_transactions(&address)
        .await
        .context("Не удалось получить транзакции")?;

    // Форматированный вывод
    DisplayFormatter::account_info(&account_info, &network);
    DisplayFormatter::transactions(&transactions);

    Ok(())
}

/// Обработка команды send
async fn handle_send(
    from: String,
    to: String,
    amount: f64,
    key_file: String,
    network: Network,
) -> Result<()> {
    log::debug!(
        "Выполнение команды send: {} -> {} ({} XRP) в сети {}",
        from,
        to,
        amount,
        network
    );

    // Валидация адресов
    if !is_valid_xrp_address(&from) {
        anyhow::bail!("Неверный адрес отправителя: {}", from);
    }
    if !is_valid_xrp_address(&to) {
        anyhow::bail!("Неверный адрес получателя: {}", to);
    }

    // Валидация суммы
    if amount <= 0.0 {
        anyhow::bail!("Сумма должна быть больше нуля");
    }
    if amount < 0.000001 {
        anyhow::bail!("Минимальная сумма отправки: 0.000001 XRP");
    }

    // Предупреждение для mainnet
    if matches!(network, Network::Mainnet) && amount > 100.0 {
        println!(
            "⚠️  ВНИМАНИЕ: Вы отправляете большую сумму ({} XRP) в MAINNET!",
            amount
        );
        println!("Это реальные деньги! Продолжить? (y/N): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("❌ Отменено пользователем");
            return Ok(());
        }
    }

    // Загружаем конфигурацию и приватный ключ
    let config = Config::load(&key_file, network)?;
    let private_key = config
        .private_key
        .ok_or_else(|| anyhow::anyhow!("Приватный ключ не найден в {}", key_file))?;

    // Декодируем приватный ключ
    let private_key_bytes =
        decode_private_key(&private_key).context("Не удалось декодировать приватный ключ")?;

    // ДОБАВЬТЕ ЭТИ СТРОКИ ДЛЯ ОТЛАДКИ:
    log::info!(
        "🔑 Размер декодированного приватного ключа: {} байт",
        private_key_bytes.len()
    );
    log::debug!(
        "   Первые 10 байт: {:?}",
        &private_key_bytes[..10.min(private_key_bytes.len())]
    );

    // Проверяем размер ключа
    if private_key_bytes.len() != 32 {
        anyhow::bail!(
            "Неверный размер приватного ключа: {} байт (ожидается 32). \
            Проверьте формат ключа в .env файле.",
            private_key_bytes.len()
        );
    }

    // Получаем публичный ключ
    let public_key =
        derive_public_key(&private_key_bytes).context("Не удалось получить публичный ключ")?;

    // Создаем API клиент
    let api = XrpApi::new(network)?;

    // Получаем информацию об аккаунте отправителя
    println!("⏳ Получение информации об аккаунте в сети {}...", network);
    let account_info = api
        .get_account_info(&from)
        .await
        .context("Не удалось получить информацию об аккаунте отправителя")?;

    // Проверяем баланс
    let balance = account_info.result.account_data.balance_xrp();
    if balance < amount + 1.0 {
        anyhow::bail!(
            "Недостаточно средств. Баланс: {} XRP, требуется: {} XRP (+ 10 XRP резерв)",
            balance,
            amount
        );
    }

    // Получаем информацию о состоянии сервера
    let server_state = api
        .get_server_state()
        .await
        .context("Не удалось получить состояние сервера")?;

    // Подготавливаем транзакцию
    let sequence = account_info.result.account_data.sequence;
    let ledger_sequence = server_state.result.state.validated_ledger.seq;
    let last_ledger_sequence = ledger_sequence + 10;

    let common_fields = TransactionCommonFields {
        transaction_type: "Payment".to_string(),
        account: from.clone(),
        fee: "12".to_string(),
        sequence,
        last_ledger_sequence: Some(last_ledger_sequence),
    };

    let payment_fields = PaymentFields {
        amount: ((amount * 1_000_000.0) as u64).to_string(),
        destination: to.clone(),
    };

    // Создаем JSON объект транзакции
    let mut tx_json = serde_json::to_value(&common_fields)?;
    let payment_json = serde_json::to_value(&payment_fields)?;
    if let (Some(tx_obj), Some(payment_obj)) = (tx_json.as_object_mut(), payment_json.as_object()) {
        tx_obj.extend(payment_obj.clone());
    }

    // Каноническая сериализация - передаем два аргумента вместо JSON
    let canonical_json = canonical_serialize(&common_fields, &payment_fields)
        .context("Не удалось выполнить каноническую сериализацию")?;

    // Подписываем транзакцию
    let signature = sign_blob(&canonical_json, &private_key_bytes)
        .context("Не удалось подписать транзакцию")?;

    // Создаем подписанную транзакцию - передаем все 5 аргументов
    let tx_blob = create_signed_tx_blob(
        canonical_json,  // Vec<u8> - без &
        signature,       // Vec<u8> - без &
        public_key,      // Vec<u8> - без &
        &common_fields,  // &TransactionCommonFields
        &payment_fields, // &PaymentFields
    )
    .context("Не удалось создать tx_blob")?;

    // Показываем детали транзакции
    println!("\n📋 Детали транзакции:");
    println!("  Сеть: {}", network);
    println!("  От: {}", from);
    println!("  Кому: {}", to);
    println!("  Сумма: {} XRP", amount);
    println!("  Комиссия: 0.000012 XRP");
    println!("  Sequence: {}", sequence);

    // Запрашиваем подтверждение
    println!("\n❓ Отправить транзакцию? (y/N): ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("❌ Отменено пользователем");
        return Ok(());
    }

    // Отправляем транзакцию
    println!("\n⏳ Отправка транзакции в сеть {}...", network);

    match api.submit_transaction(&tx_blob).await {
        Ok(result) => {
            // Проверяем результат
            if result.is_success() {
                // Успех!
                println!("\n✅ Транзакция отправлена успешно!");
                println!("🌐 Сеть: {}", network);

                if let Some(hash) = result.get_tx_hash() {
                    println!("📋 Хэш транзакции: {}", hash);
                    println!(
                        "🔍 Проверить статус: https://livenet.xrpl.org/transactions/{}",
                        hash
                    );
                }

                if let Some(engine_result) = &result.engine_result {
                    println!("📊 Статус: {}", engine_result);
                    if engine_result == "terQUEUED" {
                        println!("⏳ Транзакция добавлена в очередь и будет обработана позже");
                    }
                }

                if let Some(sequence) = result.account_sequence_next {
                    log::debug!("Следующий sequence: {}", sequence);
                }
            } else {
                // Ошибка от XRP Ledger
                println!("\n❌ Транзакция отклонена сетью XRP Ledger!");

                let error_message = result
                    .get_error_message()
                    .unwrap_or_else(|| "Неизвестная ошибка".to_string());

                println!("💬 Причина: {}", error_message);

                // Дополнительная информация об ошибке
                if let Some(engine_result) = &result.engine_result {
                    match engine_result.as_str() {
                        "tecUNFUNDED_PAYMENT" => {
                            println!("📝 Недостаточно средств на счету отправителя");
                            println!("   Проверьте баланс: cargo run -- balance {}", from);
                        }
                        "tefBAD_AUTH" => {
                            println!(
                                "📝 Неверная подпись или приватный ключ не соответствует адресу"
                            );
                            println!(
                                "   Убедитесь, что ключ в .env соответствует адресу {}",
                                from
                            );
                        }
                        "tefBAD_AUTH_MASTER" => {
                            println!("📝 Использован неправильный мастер-ключ");
                        }
                        "temBAD_SEQUENCE" => {
                            println!("📝 Неверный sequence number");
                            println!("   Возможно, есть pending транзакции");
                        }
                        "temBAD_FEE" => {
                            println!("📝 Недопустимая комиссия");
                        }
                        "temBAD_AMOUNT" => {
                            println!("📝 Недопустимая сумма");
                            println!("   Минимальная сумма: 0.000001 XRP");
                        }
                        "temDST_NEEDED" => {
                            println!("📝 Адрес получателя не активирован");
                            println!("   Для активации нового адреса требуется минимум 10 XRP");
                        }
                        "tecDST_TAG_NEEDED" => {
                            println!("📝 Для этого адреса требуется Destination Tag");
                        }
                        "tecNO_DST_INSUF_XRP" => {
                            println!("📝 Адрес получателя не может получить XRP");
                            println!("   Возможно, получатель заблокировал входящие платежи");
                        }
                        _ => {
                            // Показываем код ошибки для других случаев
                            if let Some(code) = result.engine_result_code {
                                println!("📝 Код ошибки: {} ({})", engine_result, code);
                            }
                        }
                    }
                }

                // Возвращаем ошибку для правильной обработки
                return Err(anyhow::anyhow!("Транзакция отклонена: {}", error_message));
            }
        }
        Err(e) => {
            eprintln!("\n❌ Ошибка при отправке транзакции: {}", e);

            // Дополнительный контекст ошибки
            let mut source = e.source();
            while let Some(err) = source {
                eprintln!("  ↳ {}", err);
                source = err.source();
            }

            eprintln!("\n💡 Возможные причины:");
            eprintln!("  • Проверьте подключение к интернету");
            eprintln!(
                "  • Убедитесь, что используется правильная сеть ({})",
                network
            );
            eprintln!("  • Проверьте правильность приватного ключа в .env");
            eprintln!(
                "  • Убедитесь, что адрес {} соответствует вашему ключу",
                from
            );

            if network == Network::Testnet {
                eprintln!("  • Для testnet используйте переменную XRP_SECRET_KEY_TESTNET");
            }

            return Err(e);
        }
    }
    Ok(())
}

/// Обработка команды faucet
async fn handle_faucet(address: String, network: Network) -> Result<()> {
    log::debug!(
        "Выполнение команды faucet для {} в сети {}",
        address,
        network
    );

    // Проверяем, что это не mainnet
    if matches!(network, Network::Mainnet) {
        anyhow::bail!("Faucet доступен только для testnet и devnet");
    }

    // Валидация адреса
    if !is_valid_xrp_address(&address) {
        anyhow::bail!(
            "Неверный формат XRP адреса: {}. Адрес должен начинаться с 'r'",
            address
        );
    }

    // Создаем API клиент
    let api = XrpApi::new(network)?;

    println!("⏳ Запрос тестовых XRP из faucet сети {}...", network);
    println!("   Это может занять до 30 секунд...");

    // Запрашиваем XRP из faucet
    let faucet_result = api
        .request_from_faucet(&address)
        .await
        .context("Не удалось получить XRP из faucet")?;

    // Выводим результат
    DisplayFormatter::faucet_result(&faucet_result, &network);

    // Дополнительная информация
    println!("\n💡 Подсказка:");
    println!("   Проверьте баланс через несколько секунд:");
    println!("   cargo run -- -n {} balance {}", network, address);

    Ok(())
}

/// Обработка команды network
fn handle_network_info(network: Network) {
    DisplayFormatter::network_info(&network);
}
