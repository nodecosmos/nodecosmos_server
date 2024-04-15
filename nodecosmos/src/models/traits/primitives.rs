pub trait IncrementFraction {
    /// Increment the value by the smallest possible fraction
    fn increment_fraction(self) -> Self;
}

// Not the most optimal, but keeps precision considering that f64 is not a precise type
impl IncrementFraction for f64 {
    fn increment_fraction(mut self) -> Self {
        // Determine the number of decimal places in the number
        let num_str = self.to_string();
        let decimal_places = num_str.split('.').nth(1).map_or(0, |decimals| decimals.len());

        // Calculate the smallest increment for the given number of decimal places
        let increment = 10_f64.powi(-(decimal_places as i32));

        // Return the original number plus the increment
        self = self + increment;

        self
    }
}
