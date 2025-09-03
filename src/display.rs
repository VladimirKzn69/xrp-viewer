//! Модуль для форматированного вывода информации

use crate::models::{AccountData, SubmitResult, TransactionJson};
use crate::network::Network;  // ДОБАВЛЕНО: импорт Network

/// Структура для форматированного вывода
pub struct DisplayFormatter;

impl DisplayFormatter {
    /// Отображение информации об аккаунте
    /// СОХРАНЕНА ОРИГИНАЛЬНАЯ СИГНАТУРА, добавлен опциональный network
    pub fn display_account_info(
        address: &str, 
        account_data: &AccountData,
        transaction: Option<&DisplayTransaction>,
        network: Option<Network>,  // ДОБАВЛЕНО: опциональный параметр для обратной совместимости
    ) {
        println!("\n📊 Информация об аккаунте");
        
        // ДОБАВЛЕНО: показываем сеть если передана и не mainnet
        if let Some(net) = network {
            if !net.is_production() {
                println!("  🌐 Сеть: {}", net);
            }
        }
        
        println!("  Адрес: {}", address);
        
        // Баланс в XRP (drops -> XRP: делим на 1,000,000)
        let balance_drops: u64 = account_data.balance.parse().unwrap_or(0);
        let balance_xrp = balance_drops as f64 / 1_000_000.0;
        println!("  Баланс: {:.6} XRP", balance_xrp);
        
        // Резерв
        if let Some(reserve) = &account_data.reserve {
            let reserve_drops: u64 = reserve.parse().unwrap_or(0);
            let reserve_xrp = reserve_drops as f64 / 1_000_000.0;
            println!("  Резерв: {:.6} XRP", reserve_xrp);
        }
        
        // Sequence
        println!("  Sequence: {}", account_data.sequence);
        
        // Отображение информации о последней транзакции если есть
        if let Some(tx) = transaction {
            println!("\n📄 Последняя транзакция:");
            println!("  Хэш: {}", tx.hash);
            println!("  Тип: {}", tx.transaction_type);
            if let Some(amount) = &tx.amount {
                let amount_drops: u64 = amount.parse().unwrap_or(0);
                let amount_xrp = amount_drops as f64 / 1_000_000.0;
                println!("  Сумма: {:.6} XRP", amount_xrp);
            }
            if let Some(fee) = &tx.fee {
                let fee_drops: u64 = fee.parse().unwrap_or(0);
                let fee_xrp = fee_drops as f64 / 1_000_000.0;
                println!("  Комиссия: {:.6} XRP", fee_xrp);
            }
            if let Some(destination) = &tx.destination {
                println!("  Получатель: {}", destination);
            }
        }
        
        // ДОБАВЛЕНО: ссылка на explorer если передана сеть
        if let Some(net) = network {
            let config = net.config();
            println!("\n  🔗 Explorer: {}/accounts/{}", config.explorer_url, address);
        }
    }
    
    /// Отображение результата отправки транзакции
    /// СОХРАНЕНА ОРИГИНАЛЬНАЯ СИГНАТУРА, добавлен опциональный network
    pub fn submit_result(result: &SubmitResult, network: Option<Network>) {
        println!("\n📤 Результат отправки транзакции");
        
        // ДОБАВЛЕНО: показываем сеть если передана и не mainnet
        if let Some(net) = network {
            if !net.is_production() {
                println!("  🌐 Сеть: {}", net);
            }
        }
        
        // Результат
        let status = &result.engine_result;
        if status == "tesSUCCESS" {
            println!("  ✅ Статус: УСПЕШНО");
        } else if status.starts_with("tes") {
            println!("  ✅ Статус: {}", status);
        } else if status.starts_with("ter") {
            println!("  ⚠️  Статус: {} (требуется повтор)", status);
        } else {
            println!("  ❌ Статус: {}", status);
        }
        
        // Сообщение об ошибке/результате
        if let Some(msg) = &result.engine_result_message {
            println!("  Сообщение: {}", msg);
        }
        
        // Детали транзакции если есть
        if let Some(tx_json) = &result.tx_json {
            if let Some(hash) = &tx_json.hash {
                println!("  Хэш транзакции: {}", hash);
                
                // ДОБАВЛЕНО: ссылка на explorer если передана сеть
                if let Some(net) = network {
                    let config = net.config();
                    println!("  🔗 Explorer: {}/{}", config.explorer_tx_url, hash);
                }
            }
            
            // Комиссия
            if let Some(fee) = &tx_json.fee {
                let fee_drops: u64 = fee.parse().unwrap_or(0);
                let fee_xrp = fee_drops as f64 / 1_000_000.0;
                println!("  Комиссия: {:.6} XRP", fee_xrp);
            }
        }
        
        // ДОБАВЛЕНО: предупреждение для mainnet
        if let Some(net) = network {
            if net.is_production() && status == "tesSUCCESS" {
                println!("\n  ✅ Транзакция отправлена в MAINNET (реальные XRP)!");
            }
        }
    }
    
    // НОВАЯ ФУНКЦИЯ: отображение информации о сети
    pub fn network_info(network: &Network) {
        let config = network.config();
        
        println!("\n🌐 Информация о сети");
        println!("  Название: {}", config.name);
        println!("  Тип: {}", 
            if config.is_production { 
                "Production (реальные XRP)" 
            } else { 
                "Testnet (тестовые XRP)" 
            }
        );
        
        println!("\n🔗 Endpoints");
        println!("  RPC: {}", config.rpc_url);
        if let Some(ws) = config.ws_url {
            println!("  WebSocket: {}", ws);
        }
        
        println!("\n📚 Ресурсы");
        println!("  Explorer: {}", config.explorer_url);
        if let Some(faucet) = config.faucet_url {
            println!("  Faucet: доступен");
            println!("  💡 Используйте команду 'faucet' для получения тестовых XRP");
        }
        
        if !config.is_production {
            println!("\n⚠️  Внимание");
            println!("  Это тестовая сеть - транзакции не имеют реальной стоимости!");
        } else {
            println!("\n🔴 ВАЖНО");
            println!("  Это PRODUCTION сеть с РЕАЛЬНЫМИ XRP!");
            println!("  Будьте осторожны при отправке транзакций!");
        }
    }
    
    // НОВАЯ ФУНКЦИЯ: отображение результата faucet
    pub fn faucet_result(response: &crate::api::FaucetResponse) {
        println!("\n✅ Получены тестовые XRP!");
        println!("  Адрес: {}", response.account.address);
        println!("  Сумма: {:.6} XRP", response.amount as f64 / 1_000_000.0);
        
        if let Some(balance) = response.balance {
            println!("  Новый баланс: {:.6} XRP", balance as f64 / 1_000_000.0);
        }
        
        if let Some(secret) = &response.account.secret {
            println!("\n⚠️  СОХРАНИТЕ СЕКРЕТНЫЙ КЛЮЧ");
            println!("  {}", secret);
            println!("  Этот ключ не будет показан снова!");
        }
        
        println!("\n💡 Следующие шаги:");
        println!("  1. Проверьте баланс: xrp-viewer -n testnet balance {}", response.account.address);
        println!("  2. Отправьте тестовую транзакцию");
    }
}

/// Структура для отображения транзакции
/// СОХРАНЕНА из оригинала для совместимости
pub struct DisplayTransaction {
    pub hash: String,
    pub transaction_type: String,
    pub amount: Option<String>,
    pub fee: Option<String>,
    pub destination: Option<String>,
}