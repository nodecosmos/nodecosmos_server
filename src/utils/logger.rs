use colored::Colorize;

pub fn log_success(message: String) {
    println!("{}: {}", "Success".bright_green(), message.green());
}
