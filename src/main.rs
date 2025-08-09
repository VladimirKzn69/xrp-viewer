// main.rs - точка входа в программу
use clap::Parser;
// use std::process; // Для exit - не нужен, если мы используем anyhow и возвращаем ошибку из main

// Подключаем наши модули
mod api;
mod display;
mod models;
mod config;

// Подключаем конкретные элементы из модулей
use api::XrpApi;
use display::DisplayFormatter;
// Импортируем нужные функции из config
use config::{load_env_file, get_private_key};
// Для удобной обработки ошибок
use anyhow::{Context, Result}; 

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
    log::info!("Начинаем процесс отправки...");
    log::debug!("From: {}, To: {}, Amount: {}, Key File: {}", from, to, amount, key_file);

    // 1. Загрузка .env файла
    load_env_file(&key_file)
        .with_context(|| format!("Не удалось загрузить файл конфигурации '{}'", key_file))?;

    // 2. Получение приватного ключа
    let private_key = get_private_key()
        .context("Не удалось получить приватный ключ из конфигурации")?;
    
    // 3. Вывод информации (временно, для проверки)
    log::info!("✅ .env файл '{}' успешно загружен.", key_file);
    // ВАЖНО: Никогда не выводите приватный ключ в логи или на экран!
    // Для отладки показываем только часть ключа
    if private_key.len() > 6 {
        println!("🔑 Приватный ключ загружен (первые 6 символов): {}...", &private_key[..6]);
    } else {
         println!("🔑 Приватный ключ загружен (ключ короткий).");
    }

    println!("📤 Подготовка к отправке {} XRP с {} на {}", amount, from, to);

    // 4. Создание клиента API
    let api_client = XrpApi::new().context("Не удалось создать клиент API")?;
    log::debug!("Клиент API создан.");

    // --- Здесь будет основная логика подписания и отправки ---
    // TODO: Получить sequence, fee
    // TODO: Создать транзакцию
    // TODO: Подписать транзакцию
    // TODO: Отправить транзакцию
    // TODO: Обработать результат
    // ---------------------------------------------------------

    // Пока что просто симулируем успех
    println!("✅ Транзакция успешно отправлена! (Симуляция)");
    // В реальной реализации здесь будет хэш транзакции
    // println!("Хэш транзакции: {}", transaction_hash); 

    Ok(()) // Успех
}