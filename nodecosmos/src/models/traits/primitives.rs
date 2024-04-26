use charybdis::types::Decimal;
use std::str::FromStr;

pub trait IncrementFraction {
    /// Increment the value by the smallest possible fraction
    fn increment_fraction(&mut self) -> &mut Self;
}

// TODO: update this to utilize the Decimal type without converting to string
impl IncrementFraction for Decimal {
    fn increment_fraction(&mut self) -> &mut Self {
        // Determine the number of decimal places in the number
        let num_str = self.to_string();
        // if it's whole number increment by 0.1
        if !num_str.contains('.') {
            *self = Decimal::from_str(&(num_str + ".1")).expect("Failed to parse new number");
            return self;
        }
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
