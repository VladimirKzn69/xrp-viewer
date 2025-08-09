// main.rs - точка входа в программу
use clap::Parser;
use std::process; // Для exit

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

/// Обрабатывает подкоманду 'balance'
async fn handle_balance(address: String) -> Result<(), Box<dyn std::error::Error>> {
    log::debug!("Получен адрес для баланса: {}", address);

    // Создаем клиент API
    let api_client = XrpApi::new()?;

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

                    Ok(())
                }
                Err(e) => {
                    log::error!("Ошибка получения транзакций: {}", e);
                    eprintln!("Ошибка: Не удается подключиться к API");
                    process::exit(1); 
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
            process::exit(1); 
        }
    }
}


/// Обрабатывает подкоманду 'send'
async fn handle_send(
    from: String,
    to: String,
    amount: f64,
    key_file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Начинаем процесс отправки...");
    println!("  From: {}", from);
    println!("  To: {}", to);
    println!("  Amount: {} XRP", amount);
    println!("  Key File: {}", key_file);

    // 1. Загрузка .env файла
    load_env_file(&key_file)?;

    // 2. Получение приватного ключа
    let private_key = get_private_key()?;
    println!("Приватный ключ успешно загружен.");

    // 3. Создание клиента API
    let api_client = XrpApi::new()?;
    println!("Клиент API создан.");

    // --- Здесь будет основная логика подписания и отправки ---
    // TODO: Получить sequence, fee
    // TODO: Создать транзакцию
    // TODO: Подписать транзакцию
    // TODO: Отправить транзакцию
    // TODO: Обработать результат
    // ---------------------------------------------------------

    // Пока что просто симулируем успех
    println!("Транзакция успешно отправлена! (Симуляция)");
    // В реальной реализации здесь будет хэш транзакции
    // println!("Хэш транзакции: {}", transaction_hash); 

    Ok(())
}