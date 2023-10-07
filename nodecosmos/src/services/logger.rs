use colored::Colorize;

pub fn log_success(message: String) {
    println!("{}: {}", "Success".bright_green(), message.bright_green());
}

pub fn log_warning(message: String) {
    println!("{}: {}", "Warning".bright_yellow(), message.bright_yellow());
}

pub fn log_error(message: String) {
    println!("{}: {}", "Error".red(), message.bright_red());
}

pub fn log_fatal(message: String) {
    println!(
        "{}: {}",
        "FATAL".bright_red().bold(),
        message.bright_red().bold()
    );
}
