//! Модуль для работы с конфигурацией приложения, включая загрузку переменных окружения из .env файлов.

use anyhow::{Context, Result};
use dotenv::from_filename; // Импортируем функцию из dotenv
use std::env;              // Для доступа к переменным окружения

/// Загружает переменные окружения из указанного .env файла.
/// 
/// # Аргументы
/// * `file_path` - Путь к .env файлу.
/// 
/// # Ошибки
/// Возвращает ошибку, если файл не найден или не может быть прочитан.
pub fn load_env_file(file_path: &str) -> Result<()> {
    // Пытаемся загрузить переменные из файла.
    // from_filename возвращает Result<(), dotenv::Error>
    from_filename(file_path)
        .with_context(|| format!("Не удалось загрузить .env файл: {}", file_path))?;
    log::debug!("Переменные окружения загружены из файла: {}", file_path);
    Ok(())
}

/// Получает значение переменной окружения.
/// 
/// # Аргументы
/// * `key` - Имя переменной окружения.
/// 
/// # Возвращает
/// `Some(value)`, если переменная существует, `None` если не существует.
pub fn get_env_var(key: &str) -> Option<String> {
    // env::var возвращает Result<String, VarError>
    match env::var(key) {
        Ok(value) => {
            log::debug!("Получена переменная окружения {}: ***", key); // Не логируем значение ключа!
            Some(value)
        }
        Err(e) => {
            log::debug!("Переменная окружения {} не найдена: {}", key, e);
            None
        }
    }
}

/// Получает приватный ключ из переменной окружения `PRIVATE_KEY`.
/// 
/// # Возвращает
/// `Ok(String)` с приватным ключом, если он найден.
/// `Err(...)` если переменная не найдена или пуста.
pub fn get_private_key() -> Result<String> {
    let key = get_env_var("PRIVATE_KEY")
        .context("Переменная окружения PRIVATE_KEY не найдена")?;

    if key.is_empty() {
        anyhow::bail!("Переменная окружения PRIVATE_KEY пуста");
    }

    log::debug!("Приватный ключ успешно загружен из переменной окружения");
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};
    use tempfile::NamedTempFile; // Для создания временных файлов в тестах

    #[test]
    fn test_load_env_file_and_get_private_key() -> Result<()> {
        // Создаем временный файл с содержимым
        let mut tmp_file = NamedTempFile::new()?;
        writeln!(tmp_file, "PRIVATE_KEY=s3cr3t_k3y_v4lu3")?;

        // Получаем путь к временному файлу
        let tmp_path = tmp_file.path().to_str().unwrap();

        // Загружаем переменные из временного файла
        load_env_file(tmp_path)?;

        // Получаем ключ
        let key = get_private_key()?;

        // Проверяем
        assert_eq!(key, "s3cr3t_k3y_v4lu3");

        Ok(())
    }

    #[test]
    fn test_get_private_key_not_found() {
        // Убеждаемся, что переменная не установлена
        env::remove_var("PRIVATE_KEY");

        // Попытка получить ключ должна завершиться ошибкой
        let result = get_private_key();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PRIVATE_KEY не найдена"));
    }

    #[test]
    fn test_get_private_key_empty() -> Result<()> {
        // Создаем временный файл с пустым ключом
        let mut tmp_file = NamedTempFile::new()?;
        writeln!(tmp_file, "PRIVATE_KEY=")?;

        let tmp_path = tmp_file.path().to_str().unwrap();
        load_env_file(tmp_path)?;

        // Попытка получить ключ должна завершиться ошибкой
        let result = get_private_key();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PRIVATE_KEY пуста"));

        Ok(())
    }
}