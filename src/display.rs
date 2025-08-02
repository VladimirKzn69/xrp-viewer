use crate::models::{
    DisplayAccountInfo,
    DisplayTransaction,
    AccountData,
};

pub struct DisplayFormatter;

impl DisplayFormatter {
    pub fn new() -> Self {
        DisplayFormatter
    }
    
    pub fn display_account_info(
        &self,
        address: &str,
        account_data: &AccountData,
        transaction: Option<&DisplayTransaction>
    ) {
        let display_info = DisplayAccountInfo::from_account_data(
            address.to_string(), 
            account_data
        );
        
        println!("Address: {}", self.format_address(&display_info.address));
        println!("Balance: {:.6} XRP", display_info.balance_xrp);
        println!("Last Transaction:");
        
        match transaction {
            Some(tx) => {
                println!(" Hash: {}", self.format_hash(&tx.hash));
                println!(" Amount: {:.6} XRP", tx.amount_xrp);
                println!(" Timestamp: {}", tx.timestamp);
                println!(" From: {}", self.format_address(&tx.from));
                println!(" To: {}", self.format_address(&tx.to));
            }
            None => {
                println!(" Нет транзакций.");
            }
        }
    }
    
    fn format_address(&self, address: &str) -> String {
        if address.len() <= 12 {
            address.to_string()
        } else {
            format!("{}...{}", &address[..6], &address[address.len()-3..])
        }
    }
    
    fn format_hash(&self, hash: &str) -> String {
        if hash.len() <= 12 {
            hash.to_string()
        } else {
            format!("{}...{}", &hash[..6], &hash[hash.len()-6..])
        }
    }
    #[allow(dead_code)]
    pub fn display_balance_only(&self, address: &str, account_data: &AccountData) {
        let display_info = DisplayAccountInfo::from_account_data(
            address.to_string(), 
            account_data
        );
        
        println!("Address: {}", self.format_address(address));
        println!("Balance: {:.6} XRP", display_info.balance_xrp);
    }
    
    #[allow(dead_code)]
    pub fn display_error(&self, error_message: &str) {
        eprintln!("Ошибка: {}", error_message);
    }
    
    #[allow(dead_code)]
    pub fn display_validation_info(&self, address: &str, is_valid: bool) {
        if is_valid {
            println!("Адрес {} является валидным XRP-адресом", address);
        } else {
            println!("Адрес {} НЕ является валидным XRP-адресом", address);
        }
    }
}

impl Default for DisplayFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub fn print_separator() {
    println!("{}", "-".repeat(50));
}

#[allow(dead_code)]
pub fn print_header(title: &str) {
    let separator = "=".repeat(50);
    println!("{}", separator);
    println!("{:^50}", title);
    println!("{}", separator);
}

#[allow(dead_code)]
pub trait Formattable {
    fn format(&self) -> String;
}

impl Formattable for f64 {
    fn format(&self) -> String {
        format!("{:.6} XRP", self)
    }
}

impl Formattable for String {
    fn format(&self) -> String {
        if self.len() <= 12 {
            self.clone()
        } else {
            format!("{}...{}", &self[..6], &self[self.len()-3..])
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ColoredOutput {
    pub text: String,
    pub color: OutputColor,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum OutputColor {
    Red,
    Green,
    Blue,
    Yellow,
    Default,
}

// impl ColoredOutput {
//    pub fn new(text: String, color: OutputColor) -> Self {
//        ColoredOutput { text, color }
//    }
//    
//    pub fn to_string(&self) -> String {
//        self.text.clone()
//    }
//}