use serde::{Deserialize, Serialize};

/// Money in hundredths of the account currency. Integer math keeps budget sums exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cents(pub u64);

impl Cents {
    /// Accepts a positive amount with at most two decimals.
    pub fn from_f64(v: f64) -> Option<Cents> {
        if !v.is_finite() || v <= 0.0 {
            return None;
        }
        let scaled = v * 100.0;
        let rounded = scaled.round();
        if (scaled - rounded).abs() > 1e-6 || rounded > u64::MAX as f64 / 2.0 {
            return None;
        }
        Some(Cents(rounded as u64))
    }

    pub fn format_cpc(self, decimal_comma: bool) -> String {
        self.format(decimal_comma, false)
    }

    pub fn format_budget(self, decimal_comma: bool) -> String {
        self.format(decimal_comma, true)
    }

    fn format(self, decimal_comma: bool, drop_whole: bool) -> String {
        let (whole, frac) = (self.0 / 100, self.0 % 100);
        if drop_whole && frac == 0 {
            return whole.to_string();
        }
        let sep = if decimal_comma { ',' } else { '.' };
        format!("{whole}{sep}{frac:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_f64_accepts_two_decimals() {
        assert_eq!(Cents::from_f64(1.5), Some(Cents(150)));
        assert_eq!(Cents::from_f64(50.0), Some(Cents(5000)));
        assert_eq!(Cents::from_f64(0.07), Some(Cents(7)));
    }

    #[test]
    fn from_f64_rejects_more_than_two_decimals_zero_and_negative() {
        assert_eq!(Cents::from_f64(1.234), None);
        assert_eq!(Cents::from_f64(0.0), None);
        assert_eq!(Cents::from_f64(-1.0), None);
        assert_eq!(Cents::from_f64(f64::NAN), None);
    }

    #[test]
    fn cpc_format_always_two_decimals() {
        assert_eq!(Cents(150).format_cpc(true), "1,50");
        assert_eq!(Cents(150).format_cpc(false), "1.50");
        assert_eq!(Cents(5000).format_cpc(true), "50,00");
    }

    #[test]
    fn budget_format_drops_decimals_when_whole() {
        assert_eq!(Cents(5000).format_budget(true), "50");
        assert_eq!(Cents(3750).format_budget(true), "37,50");
        assert_eq!(Cents(3750).format_budget(false), "37.50");
    }
}
