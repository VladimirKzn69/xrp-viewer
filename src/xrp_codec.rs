//src/xrp-viewer.rs

//! 🎯 Модуль xrp_codec.rs - Реализация XRP Binary Codec
//! =====================================================
//! Этот модуль реализует спецификацию бинарной сериализации XRP Ledger
//! Подробная документация: https://xrpl.org/serialization.html

use anyhow::{anyhow, Result};
use base58::FromBase58;
use sha2::{Digest, Sha256, Sha512};

// =====================================
// 📊 КОНСТАНТЫ И ТИПЫ ПОЛЕЙ XRP
// =====================================

/// Типы данных в XRP (Type Codes)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    UInt16 = 1,
    UInt32 = 2,
    UInt64 = 3,
    Hash256 = 5,
    Amount = 6,
    VL = 7, // Variable Length
    Account = 8,
    Object = 14,
    Array = 15,
    UInt8 = 16,
    Hash160 = 17,
    PathSet = 18,
    Vector256 = 19,
}

/// Field коды для транзакций Payment
/// Формат: (TypeCode, FieldCode) -> единый идентификатор
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldId {
    // Common fields
    TransactionType = 0x0102,    // (16, 2) UInt16
    Flags = 0x0202,              // (2, 2) UInt32
    SourceTag = 0x0203,          // (2, 3) UInt32
    Sequence = 0x0204,           // (2, 4) UInt32
    PreviousTxnID = 0x0205,      // (2, 5) UInt32
    LastLedgerSequence = 0x021B, // (2, 27) UInt32
    Amount = 0x0601,             // (6, 1) Amount
    Fee = 0x0608,                // (6, 8) Amount
    SendMax = 0x0609,            // (6, 9) Amount
    Account = 0x0801,            // (8, 1) Account
    Destination = 0x0803,        // (8, 3) Account
    SigningPubKey = 0x0703,      // (7, 3) VL
    TxnSignature = 0x0704,       // (7, 4) VL
    DestinationTag = 0x020E,     // (2, 14) UInt32
}

impl FieldId {
    /// Конвертировать Field ID в байты для сериализации
    pub fn to_bytes(&self) -> Vec<u8> {
        let value = *self as u16;
        // ВАЖНО: в вашем enum значения хранятся как 0xTTFF (type, field)
        let type_code = (value >> 8) & 0xFF;  // Старший байт = type
        let field_code = value & 0xFF;         // Младший байт = field
        
        // XRP Binary Codec правила:
        if type_code < 16 && field_code < 16 {
            // Однобайтовый формат: (type << 4) | field
            vec![(type_code << 4 | field_code) as u8]
        } else if type_code >= 16 && field_code < 16 {
            // Двухбайтовый: сначала field (с 0 в старших битах), затем type
            vec![field_code as u8, type_code as u8]
        } else if type_code < 16 && field_code >= 16 {
            // Двухбайтовый: type в старших 4 битах первого байта, затем field
            vec![(type_code << 4) as u8, field_code as u8]
        } else {
            // Трёхбайтовый: 0x00, type, field
            vec![0x00, type_code as u8, field_code as u8]
        }
    }
}

// =====================================
// 🔧 ОСНОВНОЙ СЕРИАЛИЗАТОР
// =====================================

pub struct XrpBinaryCodec {
    buffer: Vec<u8>,
}

impl XrpBinaryCodec {
    /// Создать новый кодек
    pub fn new() -> Self {
        XrpBinaryCodec {
            buffer: Vec::with_capacity(1024), // Предварительно выделяем память
        }
    }

    /// Добавить UInt16 поле
    pub fn append_uint16(&mut self, field: FieldId, value: u16) {
        self.buffer.extend_from_slice(&field.to_bytes());
        self.buffer.extend_from_slice(&value.to_be_bytes());
    }

    /// Добавить UInt32 поле
    pub fn append_uint32(&mut self, field: FieldId, value: u32) {
        self.buffer.extend_from_slice(&field.to_bytes());
        self.buffer.extend_from_slice(&value.to_be_bytes());
    }

    /// Добавить Amount поле (для XRP)
    pub fn append_xrp_amount(&mut self, field: FieldId, drops: u64) {
        self.buffer.extend_from_slice(&field.to_bytes());

        // XRP Amount кодируется как положительное число с установленным битом 0x4000000000000000
        // Бит 62 = 1 означает, что это XRP (не IOU token)
        // Бит 63 = 0 означает положительное число
        let xrp_amount = 0x4000000000000000u64 | drops;
        self.buffer.extend_from_slice(&xrp_amount.to_be_bytes());
    }

    /// Добавить Account поле (20 байт)
    pub fn append_account(&mut self, field: FieldId, address: &str) -> Result<()> {
        let account_id = decode_address_to_account_id(address)?;

        self.buffer.extend_from_slice(&field.to_bytes());
        self.append_variable_length(&account_id);

        Ok(())
    }

    /// Добавить Variable Length данные
    pub fn append_variable_length(&mut self, data: &[u8]) {
        let len = data.len();

        // Правила кодирования длины:
        // 0-192: один байт с значением длины
        // 193-12480: два байта (193 + len/256, len%256)
        // 12481-918744: три байта (241 + len/65536, (len/256)%256, len%256)

        if len <= 192 {
            self.buffer.push(len as u8);
        } else if len <= 12480 {
            let len1 = 193 + (len / 256);
            let len2 = len % 256;
            self.buffer.push(len1 as u8);
            self.buffer.push(len2 as u8);
        } else if len <= 918744 {
            let len1 = 241 + (len / 65536);
            let len2 = (len / 256) % 256;
            let len3 = len % 256;
            self.buffer.push(len1 as u8);
            self.buffer.push(len2 as u8);
            self.buffer.push(len3 as u8);
        } else {
            panic!("Variable length data too long: {} bytes", len);
        }

        self.buffer.extend_from_slice(data);
    }

    /// Добавить подпись и публичный ключ
    pub fn append_signature(&mut self, public_key: &[u8], signature: &[u8]) {
        // SigningPubKey (всегда 33 байта для secp256k1 compressed)
        self.buffer
            .extend_from_slice(&FieldId::SigningPubKey.to_bytes());
        self.append_variable_length(public_key);

        // TxnSignature (переменная длина DER encoding)
        self.buffer
            .extend_from_slice(&FieldId::TxnSignature.to_bytes());
        self.append_variable_length(signature);
    }

    /// Получить финальный буфер
    pub fn finalize(self) -> Vec<u8> {
        self.buffer
    }

    /// Получить буфер для подписи (без SigningPubKey и TxnSignature)
    pub fn get_signing_buffer(&self) -> Vec<u8> {
        self.buffer.clone()
    }
}

// =====================================
// 🎯 СЕРИАЛИЗАЦИЯ PAYMENT ТРАНЗАКЦИИ
// =====================================

/// Параметры для Payment транзакции
#[derive(Debug, Clone)]
pub struct PaymentTransaction {
    pub account: String,                   // Адрес отправителя
    pub destination: String,               // Адрес получателя
    pub amount: u64,                       // Сумма в drops (1 XRP = 1,000,000 drops)
    pub fee: u64,                          // Комиссия в drops
    pub sequence: u32,                     // Порядковый номер транзакции
    pub last_ledger_sequence: Option<u32>, // Максимальный ledger для включения
    pub source_tag: Option<u32>,           // Тег отправителя (опционально)
    pub destination_tag: Option<u32>,      // Тег получателя (опционально)
    pub flags: u32,                        // Флаги транзакции
}

impl PaymentTransaction {
    /// Создать базовую Payment транзакцию
    pub fn new(from: String, to: String, amount: u64, fee: u64, sequence: u32) -> Self {
        PaymentTransaction {
            account: from,
            destination: to,
            amount,
            fee,
            sequence,
            last_ledger_sequence: None,
            source_tag: None,
            destination_tag: None,
            flags: 0,
        }
    }

    /// Сериализовать транзакцию для подписи
    pub fn serialize_for_signing(&self) -> Result<Vec<u8>> {
        let mut codec = XrpBinaryCodec::new();

        // Отладка
        let tt_bytes = FieldId::TransactionType.to_bytes();
        println!("TransactionType Field ID bytes: {:?}", tt_bytes);

        // ВАЖНО: Поля должны быть в каноническом порядке!
        // Порядок определяется значением FieldId (от меньшего к большему)

        // TransactionType = 0 (Payment)
        codec.append_uint16(FieldId::TransactionType, 0);

        // Flags (если не 0)
        if self.flags != 0 {
            codec.append_uint32(FieldId::Flags, self.flags);
        }

        // SourceTag (если есть)
        if let Some(tag) = self.source_tag {
            codec.append_uint32(FieldId::SourceTag, tag);
        }

        // Sequence
        codec.append_uint32(FieldId::Sequence, self.sequence);

        // DestinationTag (если есть)
        if let Some(tag) = self.destination_tag {
            codec.append_uint32(FieldId::DestinationTag, tag);
        }

        // LastLedgerSequence (если есть)
        if let Some(lls) = self.last_ledger_sequence {
            codec.append_uint32(FieldId::LastLedgerSequence, lls);
        }

        // Amount
        codec.append_xrp_amount(FieldId::Amount, self.amount);

        // Fee
        codec.append_xrp_amount(FieldId::Fee, self.fee);

        // Account
        codec.append_account(FieldId::Account, &self.account)?;

        // Destination
        codec.append_account(FieldId::Destination, &self.destination)?;

        Ok(codec.finalize())
    }

    /// Создать полный tx_blob с подписью
    pub fn create_signed_blob(&self, public_key: &[u8], signature: &[u8]) -> Result<Vec<u8>> {
        let mut codec = XrpBinaryCodec::new();

        // Сначала все поля транзакции (как при подписи)
        // TransactionType
        codec.append_uint16(FieldId::TransactionType, 0);

        // Flags
        if self.flags != 0 {
            codec.append_uint32(FieldId::Flags, self.flags);
        }

        // SourceTag
        if let Some(tag) = self.source_tag {
            codec.append_uint32(FieldId::SourceTag, tag);
        }

        // Sequence
        codec.append_uint32(FieldId::Sequence, self.sequence);

        // DestinationTag
        if let Some(tag) = self.destination_tag {
            codec.append_uint32(FieldId::DestinationTag, tag);
        }

        // LastLedgerSequence
        if let Some(lls) = self.last_ledger_sequence {
            codec.append_uint32(FieldId::LastLedgerSequence, lls);
        }

        // Amount
        codec.append_xrp_amount(FieldId::Amount, self.amount);

        // Fee
        codec.append_xrp_amount(FieldId::Fee, self.fee);

        // SendMax (опционально, для частичных платежей)
        // Пропускаем для простой транзакции

        // Account
        codec.append_account(FieldId::Account, &self.account)?;

        // Destination
        codec.append_account(FieldId::Destination, &self.destination)?;

        // Добавляем подпись и публичный ключ
        codec.append_signature(public_key, signature);

        Ok(codec.finalize())
    }
}

// =====================================
// 🔐 ВСПОМОГАТЕЛЬНЫЕ ФУНКЦИИ
// =====================================

/// Декодировать XRP адрес в 20-байтный Account ID
/// ИСПРАВЛЕНО: используем правильную реализацию из crypto модуля
pub fn decode_address_to_account_id(address: &str) -> Result<Vec<u8>> {
    // Используем правильную реализацию с XRP Base58 из crypto модуля
    crate::crypto::decode_address_to_account_id(address)
}

/// Хэшировать данные для подписи с префиксом XRP
pub fn hash_for_signing(signing_data: &[u8]) -> Vec<u8> {
    // XRP использует префикс "STX\0" для подписания транзакций
    let prefix = b"STX\0";

    let mut hasher = Sha512::new();
    hasher.update(prefix);
    hasher.update(signing_data);
    let hash = hasher.finalize();

    // Возвращаем первые 32 байта
    hash[..32].to_vec()
}

// =====================================
// 🧪 ТЕСТЫ
// =====================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_id_encoding() {
        // TransactionType: type=16, field=2
        assert_eq!(FieldId::TransactionType.to_bytes(), vec![0x10, 0x02]);

        // Flags: type=2, field=2
        assert_eq!(FieldId::Flags.to_bytes(), vec![0x22]);

        // Account: type=8, field=1
        assert_eq!(FieldId::Account.to_bytes(), vec![0x81]);
    }

    #[test]
    fn test_variable_length_encoding() {
        let mut codec = XrpBinaryCodec::new();

        // Тест короткой длины (< 193)
        codec.append_variable_length(&vec![0u8; 50]);
        assert_eq!(codec.buffer[0], 50);

        // Тест средней длины (193-12480)
        let mut codec = XrpBinaryCodec::new();
        codec.append_variable_length(&vec![0u8; 300]);
        assert_eq!(codec.buffer[0], 194); // 193 + 300/256
        assert_eq!(codec.buffer[1], 44); // 300 % 256
    }

    #[test]
    fn test_xrp_amount_encoding() {
        let mut codec = XrpBinaryCodec::new();

        // 1 XRP = 1,000,000 drops
        codec.append_xrp_amount(FieldId::Amount, 1_000_000);

        // Проверяем, что установлен бит XRP (0x4000000000000000)
        let amount_bytes = &codec.buffer[codec.buffer.len() - 8..];
        let amount = u64::from_be_bytes(amount_bytes.try_into().unwrap());
        assert_eq!(amount, 0x4000000000000000u64 | 1_000_000);
    }

    #[test]
    fn test_address_decoding() {
        // Тестовый адрес
        let address = "rN7n7otQDd6FczFgLdSqtcsAUxDkw6fzRH";
        let account_id = decode_address_to_account_id(address);

        assert!(account_id.is_ok());
        assert_eq!(account_id.unwrap().len(), 20);
    }

    #[test]
    fn test_payment_serialization() {
        let payment = PaymentTransaction::new(
            "rN7n7otQDd6FczFgLdSqtcsAUxDkw6fzRH".to_string(),
            "rLHzPsX6oXkzU2qL12kHCH8G8cnZv1rBJh".to_string(),
            1_000_000, // 1 XRP
            12,        // 12 drops fee
            1,         // Sequence
        );

        let serialized = payment.serialize_for_signing();
        assert!(serialized.is_ok());

        let bytes = serialized.unwrap();
        assert!(bytes.len() > 0);

        // Проверяем, что начинается с TransactionType
        assert_eq!(bytes[0], 0x10); // Field ID для TransactionType
        assert_eq!(bytes[1], 0x02);
        assert_eq!(bytes[2], 0x00); // Payment = 0
        assert_eq!(bytes[3], 0x00);
    }
}

// =====================================
// 📚 ДОКУМЕНТАЦИЯ ДЛЯ ОБУЧЕНИЯ
// =====================================

/*
🎓 КАК РАБОТАЕТ XRP BINARY CODEC:

1. СТРУКТУРА ПОЛЯ:
   [Field ID][Value]

   Field ID кодируется особым образом:
   - Если type < 16 и field < 16: 1 байт (type << 4 | field)
   - Иначе: 2-3 байта

2. ПОРЯДОК ПОЛЕЙ:
   Поля ДОЛЖНЫ быть отсортированы по Field ID (canonically ordered)

3. ТИПЫ ДАННЫХ:
   - UInt8/16/32/64: Big-endian байты
   - Amount: 64 бита, где бит 62 означает XRP (не токен)
   - Account: 20 байт с VL префиксом
   - VL (Variable Length): префикс длины + данные

4. ПОДПИСЬ:
   - Сериализуем транзакцию БЕЗ SigningPubKey и TxnSignature
   - Хэшируем с префиксом "STX\0"
   - Подписываем хэш
   - Добавляем SigningPubKey и TxnSignature в конец

5. ПРИМЕР БИНАРНЫХ ДАННЫХ:
   12 00 00     - TransactionType (Payment)
   22 00000000  - Flags (0)
   24 00000001  - Sequence (1)
   61 40...     - Amount (XRP)
   68 40...     - Fee (XRP)
   81 14 ...    - Account (20 байт)
   83 14 ...    - Destination (20 байт)
   73 21 ...    - SigningPubKey (33 байта)
   74 46 ...    - TxnSignature (~72 байта)

Это НАСТОЯЩИЙ формат, используемый XRP Ledger!
*/
