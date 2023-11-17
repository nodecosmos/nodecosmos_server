use colored::Colorize;

#[allow(unused)]
pub fn log_info(message: String) {
    println!("{}: {}", "Info".bright_blue(), message.blue());
}

pub fn log_success(message: String) {
    println!("{}: {}", "Success".bright_green(), message.green());
}

pub fn log_warning(message: String) {
    println!("{}: {}", "Warning".bright_yellow(), message.yellow());
}

pub fn log_error(message: String) {
    println!("{}: {}", "Error".bright_red().bold(), message.red());
}

pub fn log_fatal(message: String) {
    println!("{}: {}", "FATAL".bright_red().bold(), message.bright_red().bold());
}
