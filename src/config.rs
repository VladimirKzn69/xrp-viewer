use anyhow::{Context, Result};
use std::env;
use std::path::Path;

use crate::network::Network;

/// Структура конфигурации приложения
#[derive(Debug, Clone)]
pub struct Config {
    pub network: Network,
    pub private_key: Option<String>,
}

impl Config {
    /// Загружает конфигурацию из файла .env
    pub fn load(env_file: &str, network: Network) -> Result<Self> {
        // Загружаем переменные окружения из файла
        load_env_file(env_file)?;

        // Получаем приватный ключ для указанной сети
        let private_key = get_private_key(&network)?;

        Ok(Self {
            network,
            private_key,
        })
    }

    /// Создает пустую конфигурацию
    pub fn empty(network: Network) -> Self {
        Self {
            network,
            private_key: None,
        }
    }
}

/// Загружает переменные окружения из .env файла
pub fn load_env_file(path: &str) -> Result<()> {
    let env_path = Path::new(path);
    
    if !env_path.exists() {
        log::warn!("Файл {} не найден", path);
        return Ok(());
    }

    dotenv::from_path(env_path)
        .with_context(|| format!("Не удалось загрузить файл {}", path))?;

    log::debug!("Конфигурация загружена из {}", path);
    Ok(())
}

/// Получает приватный ключ для указанной сети
pub fn get_private_key(network: &Network) -> Result<Option<String>> {
    // Определяем имя переменной окружения в зависимости от сети
    let env_var = network.private_key_env_var();
    
    // Пытаемся получить ключ для конкретной сети
    match env::var(&env_var) {
        Ok(key) if !key.is_empty() => {
            log::debug!("Приватный ключ найден в {}", env_var);
            Ok(Some(key))
        }
        _ => {
            // Если ключ для конкретной сети не найден, пробуем общий ключ
            match env::var("XRP_PRIVATE_KEY") {
                Ok(key) if !key.is_empty() => {
                    log::debug!("Используется общий приватный ключ XRP_PRIVATE_KEY");
                    Ok(Some(key))
                }
                _ => {
                    log::warn!("Приватный ключ не найден ни в {}, ни в XRP_PRIVATE_KEY", env_var);
                    Ok(None)
                }
            }
        }
    }
}

/// Получает значение из переменной окружения или возвращает значение по умолчанию
pub fn get_env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Проверяет наличие необходимых переменных окружения
pub fn validate_env() -> Result<()> {
    // Получаем текущую сеть
    let network_str = get_env_or_default("DEFAULT_NETWORK", "mainnet");
    let network = network_str.parse::<Network>()
        .context("Неверное значение DEFAULT_NETWORK")?;

    // Проверяем наличие приватного ключа для текущей сети
    let private_key = get_private_key(&network)?;
    if private_key.is_none() {
        log::warn!(
            "Приватный ключ не установлен. Команда 'send' будет недоступна.\n\
            Установите {} или XRP_PRIVATE_KEY в файле .env",
            network.private_key_env_var()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_load_env_file() {
        // Создаем временный файл
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "TEST_VAR=test_value").unwrap();

        // Загружаем файл
        load_env_file(file.path().to_str().unwrap()).unwrap();

        // Проверяем, что переменная загружена
        assert_eq!(env::var("TEST_VAR").unwrap(), "test_value");

        // Очищаем
        env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_get_private_key_with_network_specific() {
        // Устанавливаем переменную для testnet
        env::set_var("XRP_PRIVATE_KEY_TESTNET", "testnet_key");

        let key = get_private_key(&Network::Testnet).unwrap();
        assert_eq!(key, Some("testnet_key".to_string()));

        // Очищаем
        env::remove_var("XRP_PRIVATE_KEY_TESTNET");
    }

    #[test]
    fn test_get_private_key_fallback_to_general() {
        // Устанавливаем только общий ключ
        env::set_var("XRP_PRIVATE_KEY", "general_key");

        let key = get_private_key(&Network::Devnet).unwrap();
        assert_eq!(key, Some("general_key".to_string()));

        // Очищаем
        env::remove_var("XRP_PRIVATE_KEY");
    }

    #[test]
    fn test_config_load() {
        // Создаем временный .env файл
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "XRP_PRIVATE_KEY_TESTNET=test_private_key").unwrap();

        // Загружаем конфигурацию
        let config = Config::load(
            file.path().to_str().unwrap(),
            Network::Testnet
        ).unwrap();

        assert_eq!(config.private_key, Some("test_private_key".to_string()));
        assert!(matches!(config.network, Network::Testnet));

        // Очищаем
        env::remove_var("XRP_PRIVATE_KEY_TESTNET");
    }
}