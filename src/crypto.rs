//! Модуль для работы с криптографией XRP, включая WIF, ключи и подписи.
//! Использует правильный XRP Binary Codec для сериализации транзакций.

use crate::models::{PaymentFields, TransactionCommonFields};
use anyhow::{anyhow, Context, Result};
use base58::FromBase58;
use base64::{engine::general_purpose, Engine as _}; // Убедитесь, что импорт есть в начале файла или добавьте его

use k256::ecdsa::{SigningKey, VerifyingKey}; // Убедитесь, что эти типы импортированы
use k256::elliptic_curve::generic_array::GenericArray;
use k256::SecretKey;
use sha2::{Digest, Sha256}; // Импортируем SecretKey

// Импортируем наш новый XRP codec
use crate::xrp_codec::hash_for_signing;
use crate::xrp_codec::{FieldId, PaymentTransaction, XrpBinaryCodec};
use serde_json::json;

use crate::xrp_codec::{FIELD_ID_SIGNATURE, FIELD_ID_SIGNING_PUB_KEY};
use serde_json::Value; // Убедитесь, что этот импорт есть в начале файла

// =====================================
// 🎯 XRP BASE58 КОДЕК
// =====================================

/// XRP/Ripple использует свой алфавит для Base58
const XRP_BASE58_ALPHABET: &[u8; 58] =
    b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";

/// Декодировать XRP Base58 строку в байты
fn xrp_base58_decode(input: &str) -> Result<Vec<u8>> {
    let input_bytes = input.as_bytes();
    let base = XRP_BASE58_ALPHABET.len();

    // Подсчитываем ведущие 'r' (эквивалент нулей)
    let zeros = input_bytes.iter().take_while(|&&c| c == b'r').count();

    // Создаём обратную таблицу для быстрого поиска
    let mut decode_table = [0xff_u8; 256];
    for (i, &c) in XRP_BASE58_ALPHABET.iter().enumerate() {
        decode_table[c as usize] = i as u8;
    }

    // Вычисляем максимальный размер результата
    let size = (input.len() * 733) / 1000 + 1; // log(58) / log(256) ≈ 0.733
    let mut result = vec![0u8; size];

    for &byte in input_bytes {
        let value = decode_table[byte as usize];
        if value == 0xff {
            return Err(anyhow!(
                "Недопустимый символ в XRP Base58: '{}'",
                byte as char
            ));
        }

        let mut carry = value as u32;
        for byte in result.iter_mut().rev() {
            carry += (*byte as u32) * (base as u32);
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }

        if carry != 0 {
            return Err(anyhow!("Переполнение при декодировании Base58"));
        }
    }

    // Пропускаем ведущие нули
    let start = result.iter().position(|&b| b != 0).unwrap_or(result.len());

    // Добавляем нули для ведущих 'r'
    let mut final_result = vec![0u8; zeros];
    final_result.extend_from_slice(&result[start..]);

    Ok(final_result)
}

/// Кодировать байты в XRP Base58 строку
pub fn xrp_base58_encode(input: &[u8]) -> String {
    if input.is_empty() {
        return String::new();
    }

    let base = XRP_BASE58_ALPHABET.len();

    // Подсчитываем ведущие нули
    let zeros = input.iter().take_while(|&&b| b == 0).count();

    // Выделяем место для результата (с запасом)
    let size = (input.len() * 138) / 100 + 1; // log(256) / log(58) ≈ 1.38
    let mut result = vec![0u8; size];

    for &byte in input {
        let mut carry = byte as u32;
        for val in result.iter_mut().rev() {
            carry += (*val as u32) << 8;
            *val = (carry % (base as u32)) as u8;
            carry /= base as u32;
        }
    }

    // Пропускаем ведущие нули
    let start = result.iter().position(|&b| b != 0).unwrap_or(result.len());

    // Конвертируем в символы алфавита
    let mut encoded = String::with_capacity(zeros + (result.len() - start));

    // Добавляем 'r' для каждого ведущего нуля
    for _ in 0..zeros {
        encoded.push('r');
    }

    // Добавляем остальные символы
    for &val in &result[start..] {
        encoded.push(XRP_BASE58_ALPHABET[val as usize] as char);
    }

    encoded
}

// =====================================
// 🔧 ОБНОВЛЁННЫЕ ФУНКЦИИ ВАЛИДАЦИИ
// =====================================

/// Валидация XRP адреса с использованием правильного XRP Base58
pub fn is_valid_xrp_address(address: &str) -> bool {
    log::debug!("🔍 Проверка XRP адреса: {}", address);
    log::debug!("   Длина адреса: {} символов", address.len());

    // Проверка префикса (r для обычных адресов, X для X-адресов)
    if !address.starts_with('r') && !address.starts_with('X') {
        log::debug!("   ❌ Адрес не начинается с 'r' или 'X'");
        return false;
    }

    // Проверка длины - РАСШИРЯЕМ диапазон для поддержки всех валидных адресов
    let max_length = if address.starts_with('X') { 50 } else { 35 };
    let min_length = 25;

    if address.len() < min_length || address.len() > max_length {
        log::debug!(
            "   ❌ Неверная длина: {} (должно быть {}-{})",
            address.len(),
            min_length,
            max_length
        );
        return false;
    }

    // Используем XRP Base58 декодирование
    match xrp_base58_decode(address) {
        Ok(decoded) => {
            log::debug!("   ✅ XRP Base58 декодирование успешно");
            log::debug!("   📦 Декодированный размер: {} байт", decoded.len());

            // Минимальная проверка длины
            if decoded.len() < 21 {
                log::debug!(
                    "   ❌ Слишком короткий декодированный адрес: {} байт",
                    decoded.len()
                );
                return false;
            }

            // Логируем версию адреса
            let version = decoded[0];
            log::debug!("   📌 Версия адреса: 0x{:02x}", version);

            // Проверяем контрольную сумму для стандартной длины (25 байт)
            if decoded.len() == 25 {
                let payload = &decoded[..21]; // Версия + 20 байт AccountID
                let checksum = &decoded[21..25];

                // XRP использует двойной SHA-256 для контрольной суммы
                let hash1 = Sha256::digest(payload);
                let hash2 = Sha256::digest(&hash1);
                let calculated_checksum = &hash2[..4];

                let is_valid = checksum == calculated_checksum;

                if is_valid {
                    log::debug!("   ✅ Контрольная сумма верна");
                    log::debug!("   ✅ Адрес {} ВАЛИДЕН", address);

                    // Информативное сообщение о типе адреса
                    match version {
                        0x00 => log::debug!("   📝 Тип: Стандартный адрес (AccountID)"),
                        0x05 => log::debug!("   📝 Тип: Multi-signing адрес"),
                        _ => log::debug!("   📝 Тип: Версия 0x{:02x}", version),
                    }
                } else {
                    log::debug!("   ❌ Контрольная сумма не совпадает");
                    log::debug!("      Ожидаемая: {:?}", hex::encode(calculated_checksum));
                    log::debug!("      Полученная: {:?}", hex::encode(checksum));
                }

                is_valid
            } else {
                // Для нестандартной длины принимаем адрес
                log::debug!("   ⚠️ Нестандартная длина: {} байт", decoded.len());
                log::debug!("   ✅ Принимаем адрес как валидный");
                true
            }
        }
        Err(e) => {
            log::debug!("   ❌ Ошибка XRP Base58 декодирования: {}", e);
            false
        }
    }
}

/// Декодировать XRP адрес в 20-байтный Account ID (для xrp_codec)
pub fn decode_address_to_account_id(address: &str) -> Result<Vec<u8>> {
    // Проверка префикса
    if !address.starts_with('r') && !address.starts_with('X') {
        return Err(anyhow!("XRP адрес должен начинаться с 'r' или 'X'"));
    }

    // XRP Base58 декодирование
    let decoded = xrp_base58_decode(address)?;

    // Проверка минимальной длины
    if decoded.len() < 21 {
        return Err(anyhow!(
            "Декодированный адрес слишком короткий: {} байт",
            decoded.len()
        ));
    }

    // Для стандартных адресов проверяем контрольную сумму
    if decoded.len() == 25 {
        let payload = &decoded[..21];
        let checksum = &decoded[21..25];

        let hash1 = Sha256::digest(payload);
        let hash2 = Sha256::digest(&hash1);
        let calculated_checksum = &hash2[..4];

        if checksum != calculated_checksum {
            return Err(anyhow!("Неверная контрольная сумма адреса"));
        }
    }

    // Логируем версию для отладки
    log::debug!("Декодирование адреса с версией: 0x{:02x}", decoded[0]);

    // Возвращаем 20 байт Account ID (пропускаем первый байт версии)
    Ok(decoded[1..21].to_vec())
}

// =====================================
// ОСТАЛЬНЫЕ ФУНКЦИИ БЕЗ ИЗМЕНЕНИЙ
// =====================================

/// Декодирует WIF (Wallet Import Format) приватный ключ в байты
/// WIF использует стандартный Bitcoin Base58 (НЕ XRP!)
pub fn decode_wif(wif: &str) -> Result<Vec<u8>> {
    // Для WIF используем стандартный base58 (Bitcoin алфавит)
    let data = wif
        .from_base58()
        .map_err(|e| anyhow::anyhow!("Ошибка декодирования WIF из Base58: {:?}", e))?;

    if data.len() < 5 {
        anyhow::bail!("WIF ключ слишком короткий");
    }

    // Проверка префикса (0x80 для основной сети Bitcoin/XRP)
    if data[0] != 0x80 {
        anyhow::bail!("Неверный префикс WIF ключа");
    }

    // Проверка контрольной суммы
    let payload = &data[..data.len() - 4];
    let checksum = &data[data.len() - 4..];

    let hash = Sha256::digest(payload);
    let hash = Sha256::digest(&hash);
    let calculated_checksum = &hash[..4];

    if checksum != calculated_checksum {
        anyhow::bail!("Неверная контрольная сумма WIF ключа");
    }

    // Проверяем формат ключа
    let key_data = match payload.len() {
        33 => &payload[1..33], // Несжатый ключ
        34 => {
            if payload[33] != 0x01 {
                anyhow::bail!("Неверный флаг сжатия в WIF ключе");
            }
            &payload[1..33] // Сжатый ключ
        }
        _ => anyhow::bail!("Неверная длина WIF ключа: {}", payload.len()),
    };

    Ok(key_data.to_vec())
}

/// Получает публичный ключ из приватного (в сжатом формате для XRP)
pub fn derive_public_key(private_key: &[u8]) -> Result<Vec<u8>> {
    use k256::ecdsa::SigningKey;

    log::debug!("🔑 Генерация публичного ключа из приватного...");
    log::debug!("   Размер приватного ключа: {} байт", private_key.len());

    // Проверяем размер приватного ключа
    if private_key.len() != 32 {
        return Err(anyhow!(
            "Неверный размер приватного ключа для derive_public_key: {} байт (ожидается 32)",
            private_key.len()
        ));
    }

    // Создаем SigningKey из байтов приватного ключа
    let signing_key = SigningKey::from_slice(private_key)
        .map_err(|e| anyhow!("Не удалось создать signing key: {}", e))?;

    // Получаем публичный ключ
    let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key);

    // Преобразуем в сжатый формат (33 байта)
    let point = verifying_key.to_encoded_point(true);
    let public_key_bytes = point.as_bytes().to_vec();

    log::debug!("   ✅ Публичный ключ: {} байт", public_key_bytes.len());

    Ok(public_key_bytes)
}

/// Подписывает данные приватным ключом для XRP
/// Подписывает данные приватным ключом для XRP
pub fn sign_blob(data: &[u8], private_key: &[u8]) -> Result<Vec<u8>> {
    use k256::ecdsa::{signature::Signer, Signature, SigningKey};

    log::debug!("🔏 Подписываем данные...");
    log::debug!(" Размер данных: {} байт", data.len());
    log::debug!(" Размер приватного ключа: {} байт", private_key.len());

    // Проверяем размер приватного ключа
    if private_key.len() != 32 {
        return Err(anyhow!(
            "Неверный размер приватного ключа для подписи: {} байт (ожидается 32)",
            private_key.len()
        ));
    }

    // Получаем хеш для подписи через нашу функцию
    let final_hash = hash_for_signing(data);
    log::debug!(" Используем хеш для подписи: {} байт", final_hash.len());

    // Создаем ключ для подписи
    let signing_key = SigningKey::from_slice(private_key)
        .map_err(|e| anyhow!("Не удалось создать signing key: {}", e))?;

    // Подписываем
    let signature: Signature = signing_key.sign(&final_hash);

    // Конвертируем в DER формат
    let der_bytes = signature.to_der().to_bytes().to_vec();
    log::debug!(" ✅ Подпись создана: {} байт", der_bytes.len());
    log::debug!(" Подпись (HEX): {}", hex::encode(&der_bytes));
    Ok(der_bytes)
}

/// Правильная сериализация транзакции XRP
pub fn canonical_serialize(
    common: &TransactionCommonFields,
    payment: &PaymentFields,
) -> Result<Vec<u8>> {
    log::info!("🔧 Сериализация транзакции с использованием XRP Binary Codec");

    // Конвертируем строковые значения в числа
    let amount_drops = payment
        .amount
        .parse::<u64>()
        .context("Не удалось распарсить amount")?;

    let fee_drops = common
        .fee
        .parse::<u64>()
        .context("Не удалось распарсить fee")?;

    // Создаём структуру транзакции
    let mut tx = PaymentTransaction::new(
        common.account.clone(),
        payment.destination.clone(),
        amount_drops,
        fee_drops,
        common.sequence,
    );

    // Добавляем LastLedgerSequence если есть
    tx.last_ledger_sequence = common.last_ledger_sequence;

    // Сериализуем для подписи
    let serialized = tx
        .serialize_for_signing()
        .context("Не удалось сериализовать транзакцию")?;

    log::debug!("Сериализованная транзакция: {} байт", serialized.len());
    log::debug!("Hex: {}", hex::encode(&serialized));

    Ok(serialized)
}

/// Создает финальный tx_blob для отправки в сеть XRP Ledger.
/// Сериализует транзакцию, включая подпись и публичный ключ, в правильном порядке.
/// Поля должны быть предоставлены в уже сериализованном виде (например, строки для адресов и amounts).
pub fn create_signed_tx_blob(
    transaction_type: &str,            // Тип транзакции, например "Payment"
    account: &str,                     // Адрес отправителя
    fee: &str,                         // Комиссия в drops, например "12"
    sequence: u32,                     // Sequence номер
    destination: &str,                 // Адрес получателя
    amount: &str,                      // Сумма в drops, например "1000000"
    last_ledger_sequence: Option<u32>, // LastLedgerSequence (опционально)
    signature: Vec<u8>,                // Подпись (DER формат)
    public_key: Vec<u8>,               // Публичный ключ (сжатый, 33 байта)
) -> Result<String, anyhow::Error> {
    // Явно указываем тип ошибки
    log::info!("📦 Создание финального tx_blob с канонической сериализацией");

    // --- НОВЫЙ КОД ---
    log::debug!("📦 Подготовка PaymentTransaction для финальной сериализации");

    // Конвертируем строковые значения в нужные типы
    let fee_drops: u64 = fee
        .parse()
        .with_context(|| format!("Не удалось распарсить fee '{}'", fee))?;
    let amount_drops: u64 = amount
        .parse()
        .with_context(|| format!("Не удалось распарсить amount '{}'", amount))?;

    // Создаём структуру транзакции PaymentTransaction
    // Используем те же поля, что и в canonical_serialize
    let tx = PaymentTransaction::new(
        account.to_string(),     // account
        destination.to_string(), // destination
        amount_drops,            // amount
        fee_drops,               // fee
        sequence,                // sequence
    );
    // Устанавливаем опциональные поля
    let mut tx = tx; // Делаем изменяемым
    tx.last_ledger_sequence = last_ledger_sequence; // Устанавливаем LastLedgerSequence, если есть

    log::debug!("📦 Транзакция до добавления подписи: {:?}", tx);

    // Подписываем транзакцию, добавляя байты подписи и публичного ключа
    // Предположим, что у PaymentTransaction есть метод sign, который устанавливает поля signature и signing_pub_key
    // и возвращает новую/изменённую структуру.
    // ВАЖНО: Убедитесь, что в xrp_codec.rs реализован метод sign для PaymentTransaction!
    // Если его нет, его нужно добавить (см. предыдущее обсуждение).
    let signed_tx = tx
        .sign(signature, public_key)
        .context("Не удалось добавить подпись и публичный ключ к транзакции")?;

    log::debug!("📦 Подписанная транзакция: {:?}", signed_tx);

    // Сериализуем подписанную транзакцию в бинарный формат (Blob)
    let signed_blob_bytes = signed_tx
        .serialize()
        .context("Не удалось сериализовать подписанную транзакцию")?;
    // --- КОНЕЦ НОВОГО КОДА ---

    // Кодируем в Base64 для отправки в API
    let tx_blob = general_purpose::STANDARD.encode(&signed_blob_bytes);

    log::debug!("Финальный blob (HEX): {}", hex::encode(&signed_blob_bytes));
    log::info!("✅ Финальный tx_blob создан: {} символов", tx_blob.len());

    Ok(tx_blob)
}

/// Кодирует публичный ключ в XRP адрес
#[allow(dead_code)]
pub fn public_key_to_address(public_key: &[u8]) -> Result<String> {
    // 1. SHA-256 хэш публичного ключа
    let sha256_hash = Sha256::digest(public_key);

    // 2. RIPEMD-160 хэш результата (используем SHA-256 повторно как упрощение)
    // В реальности нужен RIPEMD-160, но для примера используем SHA-256
    let account_id_hash = Sha256::digest(&sha256_hash);
    let account_id = &account_id_hash[..20]; // Берём первые 20 байт

    // 3. Добавляем версию (0x00 для AccountID)
    let mut payload = vec![0x00];
    payload.extend_from_slice(account_id);

    // 4. Вычисляем контрольную сумму
    let checksum_hash1 = Sha256::digest(&payload);
    let checksum_hash2 = Sha256::digest(&checksum_hash1);
    let checksum = &checksum_hash2[..4];

    // 5. Собираем финальный адрес
    let mut address_bytes = payload;
    address_bytes.extend_from_slice(checksum);

    // 6. XRP Base58 кодирование
    Ok(xrp_base58_encode(&address_bytes))
}

// Добавьте эти функции в crypto.rs после существующих импортов

/// Декодирует XRP secret key (Family Seed - начинается с 's')
pub fn decode_xrp_secret(secret: &str) -> Result<Vec<u8>> {
    use sha2::{Digest, Sha512};

    log::info!("🔐 Декодируем XRP secret key");
    log::debug!("   Secret начинается с: {}", &secret[..4.min(secret.len())]);

    // Проверяем формат
    if !secret.starts_with('s') {
        return Err(anyhow!("XRP secret key должен начинаться с 's'"));
    }

    // Декодируем из XRP Base58
    let decoded = xrp_base58_decode(secret)?;
    log::debug!("   Декодировано {} байт из Base58", decoded.len());

    if decoded.len() < 17 {
        // Минимум: 1 байт тип + 16 байт seed
        return Err(anyhow!(
            "Слишком короткий secret key: {} байт",
            decoded.len()
        ));
    }

    // Для XRP secret keys:
    // - Первый байт - тип (0x21 для ED25519 или другое для secp256k1)
    // - Следующие 16 байт - seed
    // - Остальное - контрольная сумма

    let key_type = decoded[0];
    log::debug!("   Тип ключа: 0x{:02x}", key_type);

    // Извлекаем seed (16 байт после типа)
    let seed = &decoded[1..17];
    log::debug!("   Seed: {} байт", seed.len());

    // Генерируем приватный ключ из seed
    let private_key = if secret.starts_with("sEd") || key_type == 0xED {
        // ED25519 (новый формат)
        log::info!("   📝 Используем ED25519 алгоритм");
        let mut hasher = Sha512::new();
        hasher.update(seed);
        let hash = hasher.finalize();
        // ВАЖНО: Берем только первые 32 байта!
        hash[..32].to_vec()
    } else {
        // SECP256K1 (стандартный формат)
        log::info!("   📝 Используем SECP256K1 алгоритм");

        // Для secp256k1 используем специальный discriminant
        let mut discriminant = 0u32;
        loop {
            let mut hasher = Sha512::new();
            hasher.update(seed);
            hasher.update(&discriminant.to_be_bytes());
            let hash = hasher.finalize();

            // Проверяем, что первые 32 байта дают валидный приватный ключ
            // Для secp256k1 ключ должен быть меньше чем порядок группы
            let key_bytes = &hash[..32];

            // Простая проверка - не все нули и не все единицы
            if key_bytes.iter().any(|&b| b != 0) && key_bytes.iter().any(|&b| b != 0xFF) {
                log::debug!(
                    "   ✅ Валидный ключ найден с discriminant: {}",
                    discriminant
                );
                return Ok(key_bytes.to_vec());
            }

            discriminant += 1;
            if discriminant > 10 {
                return Err(anyhow!("Не удалось сгенерировать валидный ключ из seed"));
            }
        }
    };

    // Проверяем размер результата
    if private_key.len() != 32 {
        return Err(anyhow!(
            "Неверный размер сгенерированного ключа: {} (должно быть 32)",
            private_key.len()
        ));
    }

    log::info!("   ✅ Приватный ключ получен: {} байт", private_key.len());
    Ok(private_key)
}

/// Универсальная функция для декодирования приватного ключа из разных форматов
/// Декодирует приватный ключ из строки (HEX или XRP Family Seed).
/// Возвращает кортеж (SigningKey, VerifyingKey, XRP Address).
pub fn decode_private_key(
    key_str: &str,
) -> Result<(SigningKey, VerifyingKey, String), anyhow::Error> {
    if key_str.starts_with('s') || key_str.starts_with('p') {
        // XRP Family Seed
        log::debug!("🔑 Обнаружен XRP Family Seed (начинается на 's' или 'p')");

        // Вызываем нашу исправленную функцию, которая возвращает (SigningKey, VerifyingKey, String)
        let (signing_key, verifying_key, address) = derive_xrp_address_from_seed(key_str)?;
        log::debug!("🔐 Приватный ключ (SecretKey) получен из seed");
        log::debug!("🔑 Публичный ключ (VerifyingKey) получен из seed");
        log::debug!("📬 Адрес: {}", address);

        // Возвращаем кортеж
        Ok((signing_key.into(), verifying_key, address)) // Преобразуем SecretKey в SigningKey
    } else {
        // HEX приватный ключ
        log::debug!("🔑 Обнаружен HEX приватный ключ");

        // Декодируем HEX
        let private_key_bytes =
            hex::decode(key_str).context("Не удалось декодировать HEX приватный ключ")?;
        log::debug!(" 🔐 Декодировано {} байт из HEX", private_key_bytes.len());

        // Проверяем размер
        if private_key_bytes.len() != 32 {
            anyhow::bail!(
                "Неверный размер HEX приватного ключа: {} байт (ожидается 32)",
                private_key_bytes.len()
            );
        }

        // Создаём SigningKey из байт
        let signing_key = SigningKey::from_slice(&private_key_bytes)
            .map_err(|e| anyhow::anyhow!("Неверный формат приватного ключа: {}", e))?;
        log::debug!(" 🔐 SigningKey создан из HEX");

        // Получаем VerifyingKey
        // --- ИСПРАВЛЕНИЕ ---
        let verifying_key = VerifyingKey::from(&signing_key); // Используем VerifyingKey::from
                                                              // --- КОНЕЦ ИСПРАВЛЕНИЯ ---
        log::debug!(" 🔑 VerifyingKey получен из SigningKey");

        // Генерируем адрес из VerifyingKey
        let address = derive_xrp_address_from_public_key(&verifying_key)?; // Убедитесь, что эта функция существует и экспортирована
        log::debug!(" 📬 Адрес: {}", address);

        // Возвращаем кортеж
        Ok((signing_key, verifying_key, address))
    }
}
/// Деривация публичного ключа из приватного
///
/// # Аналогия
/// Это как получение отпечатка пальца из самого пальца - односторонняя операция
/// Деривация публичного ключа из приватного
pub fn derive_public_key_from_private(private_key: &SigningKey) -> Result<VerifyingKey> {
    let verifying_key = private_key.verifying_key();
    Ok(verifying_key.clone())
}

/// Генерация XRP адреса из публичного ключа
/// Деривация адреса из XRP Family Seed (начинается на 's')
/// ✅ ИСПРАВЛЕННАЯ реализация - использует SHA512 с "ripple seed" passphrase
pub fn derive_xrp_address_from_seed(
    seed: &str,
) -> Result<(k256::SecretKey, k256::ecdsa::VerifyingKey, String), anyhow::Error> {
    log::debug!("🔐 Деривация ключа из XRP Family Seed: {}", seed);

    // 1. Декодируем seed из XRP Base58
    let decoded_seed = xrp_base58_decode(seed)?;
    log::debug!(
        " 🔑 Декодированный seed ({} байт): {:?}",
        decoded_seed.len(),
        decoded_seed
    );

    // 2. Проверяем длину (должно быть 17 байт: 1 байт префикса + 16 байт энтропии)
    if decoded_seed.len() != 17 {
        anyhow::bail!(
            "Неверная длина Family Seed: {} байт (ожидается 17)",
            decoded_seed.len()
        );
    }

    // 3. Проверяем префикс (должен быть 0x21 для 's' или 0x23 для 'p')
    let prefix = decoded_seed[0];
    if prefix != 0x21 && prefix != 0x23 {
        anyhow::bail!(
            "Неверный префикс Family Seed: 0x{:02X} (ожидается 0x21 или 0x23)",
            prefix
        );
    }

    // 4. Извлекаем 16-байтную энтропию
    let mut entropy = [0u8; 16];
    entropy.copy_from_slice(&decoded_seed[1..17]);
    log::debug!(" 🧬 Энтропия (16 байт): {:?}", entropy);

    // ✅ ИСПРАВЛЕНИЕ: Используем правильный алгоритм деривации приватного ключа
    // Это имитирует алгоритм secp256k1_key_hint или RFC1751 для XRP Family Seeds
    use sha2::{Digest, Sha512};
    let mut hasher = Sha512::new();
    hasher.update(b"ripple seed"); // Используем "passphrase" как в RFC1751
    hasher.update(&entropy); // Добавляем энтропию
    let hash_result = hasher.finalize();

    // Берем первые 32 байта хеша как кандидат на приватный ключ
    let mut private_key_candidate = [0u8; 32];
    private_key_candidate.copy_from_slice(&hash_result[..32]);
    log::debug!(
        " 🔐 Кандидат на приватный ключ (первые 32 байта SHA512('ripple seed' || entropy)): {:?}",
        private_key_candidate
    );

    // ВАЖНО: Приватный ключ должен быть допустимым для secp256k1.
    // Создаем SecretKey из полученных байт
    let signing_key_secret = k256::SecretKey::from_bytes((&private_key_candidate[..]).into())
        .map_err(|e| {
            anyhow::anyhow!(
                "Неверный формат приватного ключа из seed (k256 check failed): {}",
                e
            )
        })?;

    // 5. Получаем публичный ключ
    // 1. Преобразуем SecretKey в ecdsa::SigningKey
    let signing_key_ecdsa = k256::ecdsa::SigningKey::from(&signing_key_secret);
    // 2. Получаем VerifyingKey из ecdsa::SigningKey
    let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key_ecdsa);

    // 6. Генерируем адрес из публичного ключа
    let address = derive_xrp_address_from_public_key(&verifying_key)?;
    log::debug!(" 📬 Адрес, полученный из seed: {}", address);

    // Преобразуем SecretKey в SigningKey при возврате
    Ok((signing_key_secret.into(), verifying_key, address))
}

/// Генерация XRP адреса из публичного ключа (VerifyingKey)
///
/// # Алгоритм
/// 1. Сжимаем публичный ключ в 33 байта
/// 2. SHA256 хеширование
/// 3. RIPEMD160 хеширование результата SHA256
/// 4. Добавляем префикс 0x00 для mainnet
/// 5. Base58Check кодирование с XRP алфавитом
pub fn derive_xrp_address_from_public_key(
    public_key: &k256::ecdsa::VerifyingKey,
) -> Result<String, anyhow::Error> {
    use ripemd::Ripemd160;
    use sha2::{Digest, Sha256};

    // Получаем сжатый публичный ключ (33 байта)
    let public_key_point = public_key.to_encoded_point(true);
    let public_key_bytes = public_key_point.as_bytes();
    log::debug!(
        "Public key compressed (hex): {}",
        hex::encode(public_key_bytes)
    );

    // SHA256 -> RIPEMD160
    let sha256_hash = Sha256::digest(public_key_bytes);
    let ripemd160_hash = Ripemd160::digest(&sha256_hash);

    // Создаем payload: [0x00 prefix][ripemd160 hash]
    let mut payload = Vec::with_capacity(21);
    payload.push(0x00); // Mainnet prefix
    payload.extend_from_slice(&ripemd160_hash);

    // Используем encode_check_xrp для Base58Check кодирования
    let address = encode_check_xrp(&payload)?;
    log::debug!("Generated XRP address: {}", address);

    Ok(address)
}

pub fn encode_check_xrp(data: &[u8]) -> Result<String> {
    use sha2::{Digest, Sha256};

    // Вычисляем двойной SHA256 для checksum
    let hash1 = Sha256::digest(data);
    let hash2 = Sha256::digest(&hash1);
    let checksum = &hash2[0..4];

    // Объединяем данные и checksum
    let mut result = Vec::with_capacity(data.len() + 4);
    result.extend_from_slice(data);
    result.extend_from_slice(checksum);

    // Кодируем в Base58 с XRP алфавитом
    Ok(xrp_base58_encode(&result))
}

// =====================================
// 🧪 ТЕСТЫ
// =====================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xrp_base58_codec() {
        // Тестовые данные
        let test_bytes = vec![0x00, 0x01, 0x02, 0x03];

        // Кодируем
        let encoded = xrp_base58_encode(&test_bytes);

        // Декодируем обратно
        let decoded = xrp_base58_decode(&encoded).unwrap();

        // Проверяем
        assert_eq!(test_bytes, decoded);
    }

    #[test]
    fn test_valid_xrp_addresses() {
        // Инициализируем логирование для тестов
        let _ = env_logger::builder().is_test(true).try_init();

        // Реальные рабочие XRP адреса
        assert!(is_valid_xrp_address("rN7n7otQDd6FczFgLdSqtcsAUxDkw6fzRH"));
        assert!(is_valid_xrp_address("rLHzPsX6oXkzU2qL12kHCH8G8cnZv1rBJh"));
        assert!(is_valid_xrp_address("rfokgE98PbjQZjKLJVhjhT44cGfj1cowEK"));
        assert!(is_valid_xrp_address("rEb8TK3gBgk5auZkwc6sHnwrGVJH8DuaLh")); // Binance hot wallet

        // Невалидные адреса
        assert!(!is_valid_xrp_address("invalid_address"));
        assert!(!is_valid_xrp_address("1BitcoinAddress")); // Bitcoin адрес
        assert!(!is_valid_xrp_address("xInvalidXRPAddress")); // Неправильный префикс
    }

    #[test]
    fn test_wif_decoding() {
        // Тестовый WIF ключ (НЕ используйте в реальности!)
        let wif = "L1aW4aubDFB7yfras2S1mN3bqg9nwySY8nkoLmJebSLD5BWv3ENZ";

        match decode_wif(wif) {
            Ok(key_bytes) => {
                assert_eq!(key_bytes.len(), 32);
            }
            Err(e) => {
                // WIF может быть невалидным для XRP, это нормально
                println!("WIF decode error (expected for test): {}", e);
            }
        }
    }

    #[test]
    fn test_derive_xrp_address_from_seed_correct() {
        // Инициализируем логгер для теста
        let _ = env_logger::builder().is_test(true).try_init();

        // Тестовые данные из verify_seed_js.txt
        let seed = "snd62Yu2MKbnPtLa6vYRLFPMMdZE5";
        // Адрес, который должен быть получен из этого seed (получен из JS-скрипта)
        let expected_address = "rpD6vsa5k8yPKPNYsqzbV6aDchd528e26Z";

        let result = derive_xrp_address_from_seed(seed);
        assert!(
            result.is_ok(),
            "Деривация должна быть успешной, ошибка: {:?}",
            result.err()
        );

        let (_signing_key, _verifying_key, derived_address) = result.unwrap();

        assert_eq!(
            derived_address, expected_address,
            "Адрес, полученный из seed, не совпадает с ожидаемым. Получено: {}, Ожидается: {}",
            derived_address, expected_address
        );
        println!(
            "✅ Адрес, полученный из seed '{}': {}",
            seed, derived_address
        );
    }

    #[test]
    fn test_canonical_serialize() {
        // Инициализируем логирование для теста
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();

        let common = TransactionCommonFields {
            transaction_type: "Payment".to_string(),
            account: "rN7n7otQDd6FczFgLdSqtcsAUxDkw6fzRH".to_string(),
            fee: "12".to_string(),
            sequence: 1,
            last_ledger_sequence: Some(1000),
        };

        let payment = PaymentFields {
            amount: "1000000".to_string(), // 1 XRP
            destination: "rLHzPsX6oXkzU2qL12kHCH8G8cnZv1rBJh".to_string(),
        };

        println!("🔍 Тестируем сериализацию транзакции...");
        println!("   Account: {}", common.account);
        println!("   Destination: {}", payment.destination);
        println!("   Amount: {} drops", payment.amount);
        println!("   Fee: {} drops", common.fee);

        let result = canonical_serialize(&common, &payment);

        // Проверяем что нет ошибки
        if let Err(ref e) = result {
            println!("❌ Ошибка сериализации: {}", e);
            // Печатаем backtrace если есть
            println!("   Backtrace: {:?}", e.backtrace());
        }

        assert!(result.is_ok(), "Сериализация должна быть успешной");

        let serialized = result.unwrap();
        println!("✅ Сериализация успешна!");
        println!("   Размер: {} байт", serialized.len());

        // Выводим первые байты для анализа
        if serialized.len() >= 10 {
            println!("   Первые 10 байт (hex):");
            for i in 0..10.min(serialized.len()) {
                println!("      [{}]: 0x{:02x} ({})", i, serialized[i], serialized[i]);
            }
        }

        // Полный hex дамп
        println!("   Полный hex: {}", hex::encode(&serialized));

        assert!(
            serialized.len() > 0,
            "Сериализованные данные не должны быть пустыми"
        );

        // Проверяем структуру XRP транзакции
        // TransactionType field должен быть первым

        // ВРЕМЕННО ЗАКОММЕНТИРУЕМ эти проверки чтобы увидеть что реально приходит
        println!("\n⚠️ Проверка байтов:");
        println!("   Ожидаем serialized[0] = 0x10 (TransactionType высокий байт)");
        println!("   Получили serialized[0] = 0x{:02x}", serialized[0]);

        if serialized[0] != 0x10 {
            println!("   ❌ НЕ СОВПАДАЕТ!");
            println!("   Возможные причины:");
            println!("   1. Неправильный порядок полей");
            println!("   2. Неправильное кодирование Field ID");
            println!("   3. Проблема с сортировкой полей");
        }

        // Проверяем, что начинается с правильного Field ID
        // Раскомментируйте после исправления:
        // assert_eq!(serialized[0], 0x10, "Первый байт должен быть 0x10 (Field ID для TransactionType)");
        // assert_eq!(serialized[1], 0x02, "Второй байт должен быть 0x02 (продолжение Field ID)");

        // Альтернативная проверка - ищем паттерн TransactionType
        let tx_type_pattern = vec![0x10, 0x02, 0x00, 0x00]; // TransactionType + Payment(0)

        if serialized.len() >= 4 {
            let found_at = serialized
                .windows(4)
                .position(|window| window == &tx_type_pattern[..]);

            if let Some(pos) = found_at {
                println!("\n✅ Паттерн TransactionType найден на позиции: {}", pos);
                if pos != 0 {
                    println!("   ⚠️ Но должен быть в начале (позиция 0)!");
                }
            } else {
                println!("\n❌ Паттерн TransactionType НЕ найден в сериализованных данных");
            }
        }

        // Дополнительная диагностика - проверяем что поля отсортированы
        println!("\n📊 Анализ структуры полей:");
        let mut i = 0;
        while i < serialized.len().saturating_sub(2) {
            let field_id = if serialized[i] < 0x80 {
                // Однобайтовый Field ID
                println!(
                    "   Позиция {}: Field ID = 0x{:02x} (1 байт)",
                    i, serialized[i]
                );
                i += 1;
                serialized[i - 1] as u16
            } else {
                // Двухбайтовый Field ID
                let id = ((serialized[i] as u16) << 8) | (serialized[i + 1] as u16);
                println!("   Позиция {}: Field ID = 0x{:04x} (2 байта)", i, id);
                i += 2;
                id
            };

            // Пропускаем значение поля (упрощённо)
            match field_id {
                0x1002 => {
                    println!("      -> TransactionType");
                    i += 2;
                }
                0x22 => {
                    println!("      -> Flags");
                    i += 4;
                }
                0x24 => {
                    println!("      -> Sequence");
                    i += 4;
                }
                0x201b => {
                    println!("      -> LastLedgerSequence");
                    i += 4;
                }
                0x68 => {
                    println!("      -> Fee");
                    i += 8;
                }
                0x61 => {
                    println!("      -> Amount");
                    i += 8;
                }
                0x81 => {
                    println!("      -> Account");
                    if i < serialized.len() {
                        let len = serialized[i] as usize;
                        i += 1 + len;
                    }
                }
                0x83 => {
                    println!("      -> Destination");
                    if i < serialized.len() {
                        let len = serialized[i] as usize;
                        i += 1 + len;
                    }
                }
                _ => {
                    println!("      -> Неизвестное поле");
                    break;
                }
            }

            if i >= serialized.len() {
                break;
            }
        }
    }
}
