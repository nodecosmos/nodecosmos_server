use charybdis::types::Decimal;
use std::str::FromStr;

pub trait IncrementFraction {
    /// Increment the value by the smallest possible fraction
    fn increment_fraction(&mut self) -> &mut Self;
}

impl IncrementFraction for Decimal {
    fn increment_fraction(&mut self) -> &mut Self {
        // Determine the number of decimal places in the number
        let num_str = self.to_string();
        let decimal_places = num_str.split('.').nth(1).map(|d| d.len()).unwrap_or(0);

        // Create an increment that will add exactly 1 to the last decimal place
        let increment = if decimal_places == 0 {
            Decimal::from_str("1").expect("Failed to parse increment")
        } else {
            let zeros = "0".repeat(decimal_places - 1);
            let inc = format!("0.{}1", zeros);
            Decimal::from_str(&inc).expect("Failed to parse increment")
        };

        // Add the increment
        *self += increment;

        self
    }
}
