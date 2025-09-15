use anyhow::{anyhow, Result};
use std::fmt;
use std::str::FromStr;

/// Поддерживаемые сети XRP Ledger
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Основная сеть (production)
    Mainnet,
    /// Тестовая сеть
    Testnet,
    /// Сеть для разработки
    Devnet,
}

impl Network {
    /// Возвращает конфигурацию для выбранной сети
    pub fn config(&self) -> NetworkConfig {
        match self {
            Network::Mainnet => NetworkConfig {
                name: "mainnet",
                rpc_url: "https://xrplcluster.com/", // Альтернатива: https://s1.ripple.com:51234/
                ws_url: Some("wss://xrplcluster.com/"),
                explorer_url: "https://livenet.xrpl.org",
                explorer_tx_url: "https://livenet.xrpl.org/transactions",
                faucet_url: None,
                is_production: true,
            },
            Network::Testnet => NetworkConfig {
                name: "testnet",
                rpc_url: "https://testnet.xrpl-labs.com/", // <-- ЗАМЕНИЛИ URL
                ws_url: Some("wss://testnet.xrpl-labs.com/"), // <-- И WebSocket тоже
                explorer_url: "https://testnet.xrpl.org",
                explorer_tx_url: "https://testnet.xrpl.org/transactions",
                faucet_url: Some("https://faucet.altnet.rippletest.net"),
                is_production: false,
            },
            Network::Devnet => NetworkConfig {
                name: "devnet",
                rpc_url: "https://s.devnet.rippletest.net:51234/",
                ws_url: Some("wss://s.devnet.rippletest.net:51233"),
                explorer_url: "https://devnet.xrpl.org",
                explorer_tx_url: "https://devnet.xrpl.org/transactions",
                faucet_url: Some("https://faucet.devnet.rippletest.net"),
                is_production: false,
            },
        }
    }

    /// Возвращает имя переменной окружения для приватного ключа
    pub fn private_key_env_var(&self) -> String {
        match self {
            Network::Mainnet => "XRP_PRIVATE_KEY_MAINNET".to_string(),
            Network::Testnet => "XRP_PRIVATE_KEY_TESTNET".to_string(),
            Network::Devnet => "XRP_PRIVATE_KEY_DEVNET".to_string(),
        }
    }
}

impl Default for Network {
    fn default() -> Self {
        Network::Mainnet
    }
}

impl FromStr for Network {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "mainnet" | "main" | "production" | "prod" => Ok(Network::Mainnet),
            "testnet" | "test" => Ok(Network::Testnet),
            "devnet" | "dev" | "development" => Ok(Network::Devnet),
            _ => Err(anyhow!(
                "Неизвестная сеть: {}. Доступны: mainnet, testnet, devnet",
                s
            )),
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.config().name)
    }
}

/// Конфигурация сети
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Название сети
    pub name: &'static str,
    /// RPC endpoint URL
    pub rpc_url: &'static str,
    /// WebSocket endpoint URL (если доступен)
    pub ws_url: Option<&'static str>,
    /// URL блокчейн эксплорера
    pub explorer_url: &'static str,
    /// URL для просмотра транзакций
    pub explorer_tx_url: &'static str,
    /// URL faucet для получения тестовых XRP
    pub faucet_url: Option<&'static str>,
    /// Является ли это production сетью
    pub is_production: bool,
}

impl NetworkConfig {
    /// Получить полный URL для просмотра транзакции
    pub fn transaction_url(&self, tx_hash: &str) -> String {
        format!("{}/{}", self.explorer_tx_url, tx_hash)
    }

    /// Получить полный URL для просмотра адреса
    pub fn address_url(&self, address: &str) -> String {
        format!("{}/accounts/{}", self.explorer_url, address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_from_str() {
        assert_eq!(Network::from_str("mainnet").unwrap(), Network::Mainnet);
        assert_eq!(Network::from_str("TESTNET").unwrap(), Network::Testnet);
        assert_eq!(Network::from_str("dev").unwrap(), Network::Devnet);
        assert!(Network::from_str("invalid").is_err());
    }

    #[test]
    fn test_network_config() {
        let mainnet = Network::Mainnet.config();
        assert!(mainnet.is_production);
        assert!(mainnet.faucet_url.is_none());

        let testnet = Network::Testnet.config();
        assert!(!testnet.is_production);
        assert!(testnet.faucet_url.is_some());
    }

    #[test]
    fn test_private_key_env_var() {
        assert_eq!(
            Network::Mainnet.private_key_env_var(),
            "XRP_PRIVATE_KEY_MAINNET"
        );
        assert_eq!(
            Network::Testnet.private_key_env_var(),
            "XRP_PRIVATE_KEY_TESTNET"
        );
    }

    #[test]
    fn test_testnet_url() {
        let testnet = Network::Testnet.config();
        // Проверяем что URL содержит порт
        assert!(testnet.rpc_url.contains(":51234"));
        assert_eq!(testnet.rpc_url, "https://s.altnet.rippletest.net:51234/");
    }
}
