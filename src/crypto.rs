//! Модуль для работы с криптографией XRP, включая WIF, ключи и подписи.
use anyhow::{Context, Result};
use k256::{
    ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey},
    EncodedPoint, SecretKey,
};
use sha2::{Sha512, Digest};
use base58::{FromBase58, ToBase58};
use std::collections::BTreeMap;
use serde_json::{json, Value};
use crate::models::{TransactionCommonFields, PaymentFields}; // Предполагаем, что модели находятся в models.rs

/// Декодирует WIF (Wallet Import Format) приватный ключ в байты
pub fn decode_wif(wif: &str) -> Result<Vec<u8>> {
    let data = wif.from_base58()
        .map_err(|e| anyhow::anyhow!("Ошибка декодирования WIF из Base58: {:?}", e))?;
    if data.len() < 5 {
        anyhow::bail!("WIF ключ слишком короткий");
    }
    // Проверка префикса (0x80 для основной сети)
    if data[0] != 0x80 {
        anyhow::bail!("Неверный префикс WIF ключа");
    }
    // Проверка контрольной суммы
    let payload = &data[..data.len() - 4];
    let checksum = &data[data.len() - 4..];
    let hash = Sha512::digest(payload);
    let hash = Sha512::digest(&hash[..]);
    let calculated_checksum = &hash[..4];
    if checksum != calculated_checksum {
        anyhow::bail!("Неверная контрольная сумма WIF ключа");
    }
    // Извлекаем 32-байтный приватный ключ (без префикса и контрольной суммы)
    let private_key_bytes = &payload[1..33]; // [1..33] - это 32 байта
    Ok(private_key_bytes.to_vec())
}

/// Получает публичный ключ из приватного (в байтах)
pub fn derive_public_key(private_key_bytes: &[u8]) -> Result<Vec<u8>> {
    // 1. Создаем SecretKey из байтов
    let secret_key = SecretKey::from_bytes(private_key_bytes.into())
        .map_err(|e| anyhow::anyhow!("Ошибка создания SecretKey: {:?}", e))?; // <-- {:?} для ошибки

    // 2. Создаем SigningKey из SecretKey
    let signing_key = SigningKey::from(secret_key);

    // 3. Получаем VerifyingKey из SigningKey
    let verifying_key = signing_key.verifying_key();

    // 4. Кодируем публичный ключ в несжатом формате
    let encoded_point = verifying_key.to_encoded_point(false); // false = uncompressed

    Ok(encoded_point.as_bytes().to_vec())
}

/// Подписывает данные (бинарный blob) приватным ключом
pub fn sign_blob(private_key_bytes: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    let signing_key = SigningKey::from_bytes(private_key_bytes.into())
        .map_err(|e| anyhow::anyhow!("Ошибка создания SigningKey: {}", e))?;
    let signature: Signature = signing_key.sign(blob);
    // k256::ecdsa::Signature уже в DER-формате, когда сериализуется через `to_vec()`
    Ok(signature.to_vec())
}

// --- Функция для каноничного кодирования (упрощённая заглушка) ---
// Реальная реализация требует знания XRPL Serialization Format (SHAMap, Field IDs и т.д.)
// Это очень сложная часть. Для учебного проекта мы сделаем упрощённую версию,
// которая просто сериализует BTreeMap в JSON и хеширует его.
// В реальности нужно использовать библиотеку или реализовать спецификацию XRPL.

/// Упрощённая каноническая сериализация транзакции (заглушка).
/// В реальности используется собственный бинарный формат XRPL.
pub fn canonical_serialize_stub(common: &TransactionCommonFields, payment: &PaymentFields) -> Result<Vec<u8>> {
    log::warn!("⚠️  Используется УПРОЩЕННОЕ кодирование транзакции (заглушка)!");
    // Создаем BTreeMap для хранения всех полей в каноничном (отсортированном) порядке
    let mut tx_map = BTreeMap::new();
    // Добавляем поля из common_fields
    tx_map.insert("TransactionType".to_string(), Value::String(common.transaction_type.clone()));
    tx_map.insert("Account".to_string(), Value::String(common.account.clone()));
    tx_map.insert("Fee".to_string(), Value::String(common.fee.clone()));
    tx_map.insert("Sequence".to_string(), Value::Number(common.sequence.into()));
    if let Some(lls) = common.last_ledger_sequence {
         tx_map.insert("LastLedgerSequence".to_string(), Value::Number(lls.into()));
    }
    // Добавляем поля из payment_fields
    tx_map.insert("Amount".to_string(), Value::String(payment.amount.clone()));
    tx_map.insert("Destination".to_string(), Value::String(payment.destination.clone()));
    // Сериализуем в JSON
    let json_string = serde_json::to_string(&tx_map)
        .map_err(|e| anyhow::anyhow!("Ошибка сериализации транзакции в JSON: {}", e))?;
    log::debug!("Сериализованная транзакция (JSON): {}", json_string);
    // В реальности XRPL использует собственный бинарный формат, а не JSON.
    // Здесь мы просто хешируем JSON-строку как пример.
    // let mut hasher = Sha512::new();
    // hasher.update(json_string.as_bytes());
    // let hash = hasher.finalize();
    // Ok(hash[..32].to_vec()) // Только первые 32 байта хэша SHA512
    // Для демонстрации просто возвращаем байты JSON-строки
    Ok(json_string.into_bytes())
}

/// Создаёт финальный tx_blob (заглушка).
/// В реальности нужно снова сериализовать транзакцию с добавленными SigningPubKey и TxnSignature
/// и затем закодировать её в XRPL binary format.
pub fn create_signed_tx_blob_stub(signed_blob: Vec<u8>, signature_der: Vec<u8>, public_key: Vec<u8>) -> Result<String> {
    log::warn!("⚠️  Используется УПРОЩЕННОЕ создание tx_blob (заглушка)!");
    // Здесь мы просто конкатенируем signature и public_key к blob и кодируем в HEX
    // Это НЕПРАВИЛЬНЫЙ tx_blob, но подходит для демонстрации потока.
    let mut final_blob = signed_blob;
    final_blob.extend_from_slice(&signature_der);
    final_blob.extend_from_slice(&public_key);
    Ok(hex::encode(final_blob))
}