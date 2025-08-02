// main.rs - точка входа в программу
use clap::Parser;

// Подключаем наши модули
mod address;
mod api;
mod display;
mod models;

// Подключаем конкретные элементы из модулей
use address::{AddressValidator, AddressError};
use api::XrpApi;
use display::DisplayFormatter;

/// CLI-приложение для просмотра баланса XRP-кошелька
#[derive(Debug, Parser)]
#[clap(name = "xrp-viewer", version = "0.1.0", about = "Просмотр баланса XRP-кошелька")]
struct Cli {
    /// Публичный XRP-адрес (начинается с 'r')
    #[clap(value_parser)]
    address: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация логирования
    env_logger::init();
    
    // Парсинг аргументов командной строки
    let cli = Cli::parse();
    
    log::debug!("Получен адрес: {}", cli.address);
    
    // Создаем валидатор адреса
    let validator = AddressValidator::new();
    
    // Валидация адреса с подробным логированием
    log::debug!("Начинаем валидацию адреса...");
    match validator.validate(&cli.address) {
        Ok(()) => {
            log::info!("Адрес {} прошел валидацию", cli.address);
            
            // Создаем клиент API
            let api_client = XrpApi::new()?;
            
            // Получаем информацию о кошельке
            match api_client.get_account_info(&cli.address).await {
                Ok(account_info) => {
                    log::info!("Получена информация о кошельке");
                    
                    // Получаем последние транзакции
                    match api_client.get_latest_transaction(&cli.address).await {
                        Ok(transaction) => {
                            // Создаем форматировщик вывода
                            let formatter = DisplayFormatter::new();
                            
                            // Выводим информацию
                            formatter.display_account_info(
                                &cli.address, 
                                &account_info.result.account_data, 
                                transaction.as_ref()
                            );
                            
                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Ошибка получения транзакций: {}", e);
                            eprintln!("Ошибка: Не удается подключиться к API");
                            Ok(())
                        }
                    }
                }
                Err(e) => {
                    log::error!("Ошибка API: {}", e);
                    if e.to_string().contains("actNotFound") || e.to_string().contains("Account not found") {
                        eprintln!("Ошибка: Кошелек не найден");
                    } else {
                        eprintln!("Ошибка: Не удается подключиться к API");
                    }
                    Ok(())
                }
            }
        }
        Err(error) => {
            log::error!("Ошибка валидации адреса {}: {:?}", cli.address, error);
            match error {
                AddressError::TooShort => {
                    eprintln!("Ошибка: Адрес слишком короткий");
                }
                AddressError::TooLong => {
                    eprintln!("Ошибка: Адрес слишком длинный");
                }
                AddressError::InvalidPrefix => {
                    eprintln!("Ошибка: Адрес должен начинаться с 'r'");
                }
                AddressError::InvalidBase58 => {
                    eprintln!("Ошибка: Некорректный формат адреса");
                }
                AddressError::InvalidChecksum => {
                    eprintln!("Ошибка: Некорректный формат адреса");
                }
                AddressError::InvalidDataLength => {
                    eprintln!("Ошибка: Некорректный формат адреса");
                }
            }
            Ok(())
        }
    }
}