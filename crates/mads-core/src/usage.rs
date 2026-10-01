use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Only set when the provider reports a cost.
    pub cost_usd: Option<f64>,
}

impl Usage {
    pub fn add(&mut self, other: &Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cost_usd = match (self.cost_usd, other.cost_usd) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
        };
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sums_tokens() {
        let mut a = Usage {
            input_tokens: 10,
            output_tokens: 5,
            cost_usd: None,
        };
        a.add(&Usage {
            input_tokens: 1,
            output_tokens: 2,
            cost_usd: None,
        });
        assert_eq!((a.input_tokens, a.output_tokens), (11, 7));
        assert_eq!(a.total_tokens(), 18);
    }

    #[test]
    fn cost_is_known_when_any_side_reports_it() {
        let mut a = Usage::default();
        a.add(&Usage {
            cost_usd: Some(0.5),
            ..Usage::default()
        });
        assert_eq!(a.cost_usd, Some(0.5));
        a.add(&Usage {
            cost_usd: None,
            ..Usage::default()
        });
        assert_eq!(a.cost_usd, Some(0.5));
        a.add(&Usage {
            cost_usd: Some(0.25),
            ..Usage::default()
        });
        assert_eq!(a.cost_usd, Some(0.75));
    }
}
