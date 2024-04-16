use charybdis::types::Decimal;
use std::str::FromStr;

pub trait IncrementFraction {
    /// Increment the value by the smallest possible fraction
    fn increment_fraction(&mut self) -> &mut Self;
}

// Not the most optimal, but keeps precision considering that f64 is not a precise type
impl IncrementFraction for Decimal {
    fn increment_fraction(&mut self) -> &mut Self {
        // Determine the number of decimal places in the number
        let num_str = self.to_string();
        // parse last digit as u8, increment it, and convert back to string
        let last_digit = num_str.chars().last().unwrap().to_digit(10).unwrap();
        let new_last_digit = last_digit + 1;
        let new_num_str = num_str
            .chars()
            .take(num_str.len() - 1)
            .chain(std::char::from_digit(new_last_digit, 10))
            .collect::<String>();

        *self = Decimal::from_str(&new_num_str).expect("Failed to parse new number");

        self
    }
}
