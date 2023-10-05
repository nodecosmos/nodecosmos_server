use colored::Colorize;

pub(crate) fn log_warning(message: String) {
    println!("{}: {}", "Warning".bright_yellow(), message.bright_yellow());
}

pub(crate) fn log_error(message: String) {
    println!("{}: {}", "Error".red(), message.bright_red());
}

pub(crate) fn log_fatal(message: String) {
    println!(
        "{}: {}",
        "FATAL".bright_red().bold(),
        message.bright_red().bold()
    );
}
