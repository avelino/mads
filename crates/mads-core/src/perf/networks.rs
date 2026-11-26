//! Where each campaign's cost and conversions came from, from the Campaigns report segmented by
//! network. Clicks on the Display network of an App campaign are the ones to doubt.

use serde::{Deserialize, Serialize};

use super::model::{CampaignPerf, Metrics, Performance};
use crate::input::fold;

/// A Display CTR above this, in percent, on an App campaign reads as accidental clicks: a
/// banner in an app is tapped by mistake far more than a result on a search page. mads draws
/// this line, Google has none.
pub const DISPLAY_CTR_PCT: f64 = 15.0;
/// Display clicks needed before its CTR says anything: a few clicks on a few impressions can
/// reach any rate. Also a line mads draws.
pub const DISPLAY_MIN_CLICKS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Search,
    SearchPartners,
    Display,
    #[serde(rename = "youtube")]
    YouTube,
    Gmail,
    Maps,
    Discover,
    /// Mixed or cross-network rows, and any name mads does not know.
    Other,
}

impl Network {
    /// The network a row names, in English or Portuguese: `Search Partners`,
    /// `Parceiros de pesquisa`, `Display Network`, `Rede de Display`, `Content`.
    pub fn of(text: &str) -> Network {
        let t = fold(text);
        let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
        if has(&["partner", "parceiro"]) {
            Network::SearchPartners
        } else if has(&["youtube"]) {
            Network::YouTube
        } else if has(&["display", "content", "conteudo"]) {
            Network::Display
        } else if has(&["gmail"]) {
            Network::Gmail
        } else if has(&["maps", "mapas"]) {
            Network::Maps
        } else if has(&["discover", "descoberta"]) {
            Network::Discover
        } else if has(&["search", "pesquisa"]) {
            Network::Search
        } else {
            Network::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Network::Search => "search",
            Network::SearchPartners => "search partners",
            Network::Display => "display",
            Network::YouTube => "youtube",
            Network::Gmail => "gmail",
            Network::Maps => "maps",
            Network::Discover => "discover",
            Network::Other => "other",
        }
    }
}

/// One network of one campaign: its share of the campaign's cost and conversions, and its CTR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkPerf {
    pub network: Network,
    /// Percent of the campaign's cost. None when the campaign cost nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_share_pct: Option<f64>,
    /// Percent of the campaign's conversions. None when it had none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversions_share_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctr_pct: Option<f64>,
}

/// The rows of one campaign by network, added up per network: the shares, the most expensive
/// network first, and whether its Display clicks look accidental. Only an App campaign can have
/// that flag.
pub(super) fn summarize(rows: &[(Network, Metrics)], app: bool) -> (Vec<NetworkPerf>, bool) {
    let mut by: Vec<(Network, Metrics)> = Vec::new();
    for (network, m) in rows {
        match by.iter_mut().find(|(n, _)| n == network) {
            Some((_, total)) => total.add(m),
            None => by.push((*network, m.clone())),
        }
    }
    by.retain(|(_, m)| m.impressions > 0 || m.cost > 0.0 || m.conversions > 0.0);
    let cost: f64 = by.iter().map(|(_, m)| m.cost).sum();
    let conversions: f64 = by.iter().map(|(_, m)| m.conversions).sum();
    let share = |part: f64, total: f64| (total > 0.0).then(|| round1(part / total * 100.0));
    let accidental = app
        && by
            .iter()
            .any(|(n, m)| *n == Network::Display && accidental_clicks(m));
    by.sort_by(|(an, a), (bn, b)| b.cost.total_cmp(&a.cost).then(an.cmp(bn)));
    let networks = by
        .iter()
        .map(|(network, m)| NetworkPerf {
            network: *network,
            cost_share_pct: share(m.cost, cost),
            conversions_share_pct: share(m.conversions, conversions),
            ctr_pct: m.ctr_pct,
        })
        .collect();
    (networks, accidental)
}

/// Display clicks enough to read, at a CTR above `DISPLAY_CTR_PCT`. At the line it is not flagged.
fn accidental_clicks(display: &Metrics) -> bool {
    display.clicks >= DISPLAY_MIN_CLICKS && display.ctr_pct.is_some_and(|c| c > DISPLAY_CTR_PCT)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn describe(n: &NetworkPerf) -> String {
    let mut parts = Vec::new();
    if let Some(pct) = n.cost_share_pct {
        parts.push(format!("{pct}% of cost"));
    }
    if let Some(pct) = n.conversions_share_pct {
        parts.push(format!("{pct}% of conversions"));
    }
    if let Some(ctr) = n.ctr_pct {
        parts.push(format!("CTR {ctr}%"));
    }
    format!("{} {}", n.network.label(), parts.join(", "))
}

fn display_ctr(c: &CampaignPerf) -> Option<f64> {
    c.networks
        .iter()
        .find(|n| n.network == Network::Display)
        .and_then(|n| n.ctr_pct)
}

impl Performance {
    /// One line per campaign that ran on more than one network, and one per App campaign whose
    /// Display clicks look accidental.
    pub(super) fn network_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in self.campaigns.iter().filter(|c| c.networks.len() > 1) {
            let nets: Vec<String> = c.networks.iter().map(describe).collect();
            out.push(format!("networks of {}: {}", c.name, nets.join("; ")));
        }
        for c in self
            .campaigns
            .iter()
            .filter(|c| c.accidental_display_clicks)
        {
            let ctr = display_ctr(c).unwrap_or_default();
            out.push(format!(
                "Display CTR of {} is {ctr}%, above {DISPLAY_CTR_PCT}%: likely accidental clicks, so treat its installs with caution and do not raise its budget on cost per install alone",
                c.name
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        network: Network,
        impressions: u64,
        clicks: u64,
        cost: f64,
        conv: f64,
    ) -> (Network, Metrics) {
        let mut m = Metrics {
            impressions,
            clicks,
            cost,
            conversions: conv,
            ..Default::default()
        };
        m.derive();
        (network, m)
    }

    #[test]
    fn network_names_read_in_english_and_portuguese() {
        for (text, want) in [
            ("Google search", Network::Search),
            ("Pesquisa Google", Network::Search),
            ("Search Partners", Network::SearchPartners),
            ("Parceiros de pesquisa", Network::SearchPartners),
            ("Display Network", Network::Display),
            ("Rede de Display", Network::Display),
            ("Content", Network::Display),
            ("YouTube Videos", Network::YouTube),
            ("YouTube Search", Network::YouTube),
            ("Gmail", Network::Gmail),
            ("Maps", Network::Maps),
            ("Discover", Network::Discover),
            ("Cross-network", Network::Other),
        ] {
            assert_eq!(Network::of(text), want, "{text}");
        }
    }

    #[test]
    fn shares_add_up_per_network_and_the_costliest_comes_first() {
        let rows = [
            row(Network::Search, 1000, 100, 60.0, 9.0),
            row(Network::Display, 1000, 50, 30.0, 1.0),
            row(Network::Search, 1000, 100, 10.0, 0.0),
        ];
        let (nets, flag) = summarize(&rows, false);
        assert!(!flag);
        assert_eq!(nets.len(), 2);
        assert_eq!(nets[0].network, Network::Search);
        assert_eq!(nets[0].cost_share_pct, Some(70.0));
        assert_eq!(nets[0].conversions_share_pct, Some(90.0));
        assert_eq!(nets[0].ctr_pct, Some(10.0));
        assert_eq!(nets[1].network, Network::Display);
        assert_eq!(nets[1].cost_share_pct, Some(30.0));
        assert_eq!(nets[1].ctr_pct, Some(5.0));
    }

    #[test]
    fn a_campaign_without_cost_or_conversions_has_no_shares() {
        let (nets, _) = summarize(&[row(Network::Search, 100, 5, 0.0, 0.0)], false);
        assert_eq!(nets[0].cost_share_pct, None);
        assert_eq!(nets[0].conversions_share_pct, None);
        assert_eq!(nets[0].ctr_pct, Some(5.0));
        let (empty, _) = summarize(&[row(Network::Maps, 0, 0, 0.0, 0.0)], false);
        assert!(empty.is_empty(), "a network with no traffic is left out");
    }

    #[test]
    fn display_ctr_is_flagged_above_the_line_and_not_at_it() {
        let flagged = |clicks: u64, impressions: u64, app: bool| {
            summarize(
                &[row(Network::Display, impressions, clicks, 10.0, 1.0)],
                app,
            )
            .1
        };
        assert!(!flagged(150, 1000, true), "15.0 is at the line");
        assert!(flagged(151, 1000, true), "15.1 is above it");
        assert!(
            !flagged(151, 1000, false),
            "only App campaigns carry the flag"
        );
    }

    #[test]
    fn display_ctr_needs_clicks_enough_to_read() {
        let few = DISPLAY_MIN_CLICKS - 1;
        let (_, flag) = summarize(&[row(Network::Display, few + 1, few, 1.0, 0.0)], true);
        assert!(!flag, "{few} clicks are noise");
        let (_, flag) = summarize(
            &[row(
                Network::Display,
                DISPLAY_MIN_CLICKS + 1,
                DISPLAY_MIN_CLICKS,
                1.0,
                0.0,
            )],
            true,
        );
        assert!(flag);
    }

    #[test]
    fn a_high_ctr_on_search_is_not_flagged() {
        let (_, flag) = summarize(&[row(Network::Search, 100, 60, 10.0, 1.0)], true);
        assert!(!flag);
    }
}
