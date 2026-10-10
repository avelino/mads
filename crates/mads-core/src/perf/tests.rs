use super::*;
use crate::google::{
    Account, AdGroup, BidStrategy, Campaign, Cents, Intent, Keyword, MatchType, Rsa,
};

fn kw(text: &str, match_type: MatchType) -> Keyword {
    Keyword {
        text: text.into(),
        match_type,
    }
}

fn campaign(name: &str, groups: &[(&str, Vec<Keyword>)]) -> Campaign {
    Campaign {
        kind: Default::default(),
        asset_groups: Vec::new(),
        name: name.into(),
        slug: crate::input::slugify(name),
        intent: Intent::Generic,
        daily_budget: Cents(5000),
        bid_strategy: BidStrategy::ManualCpc,
        rationale: "r".into(),
        planned_ad_groups: Vec::new(),
        ad_groups: groups
            .iter()
            .map(|(g, keywords)| AdGroup {
                name: (*g).into(),
                default_cpc: Cents(120),
                cpc_rationale: String::new(),
                final_url: String::new(),
                keywords: keywords.clone(),
                negatives: Vec::new(),
                rsa: Rsa::default(),
            })
            .collect(),
        negatives: vec![kw("emprego", MatchType::Phrase)],
        assets: None,
    }
}

fn account() -> Account {
    Account {
        brand_kit: None,
        campaigns: vec![
            campaign(
                "Catálogo: uvas",
                &[(
                    "Uvas tintas",
                    vec![
                        kw("malbec", MatchType::Phrase),
                        kw("malbec", MatchType::Exact),
                    ],
                )],
            ),
            campaign("Marca Vinellu", &[("Vinellu marca", vec![])]),
        ],
    }
}

const CAMPAIGNS: &str = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Orçamento,Status,Motivos do status,Custo,Conversões,Impr.,Cliques\nAtivada,\"Catálogo: uvas\",\"70,00\",Qualificado (limitado),a maioria dos anúncios foi limitada pela política,\"178,93\",\"0,00\",\"5.874\",156\nAtivada,Marca Vinellu,\"25,00\",Qualificado,,\"0,00\",\"0,00\",0,0\nPausada,Rotulos antigo,\"250,00\",Pausado,campanha pausada,\"0,00\",\"0,00\",0,0\nTotal: Conta,,,,,\"178,93\",\"0,00\",\"5.874\",156\n";

const KEYWORDS: &str = "Relatório de palavras-chave da rede de pesquisa\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da palavra-chave,Palavra-chave,Tipo de corresp.,Campanha,Grupo de anúncios,Status,Motivos do status,CPC máx.,Impr.,Custo,Cliques,Conversões\nAtivado,\"\"\"malbec\"\"\",Correspondência de frase,\"Catálogo: uvas\",Uvas tintas,Limitado,abaixo do lance de primeira página,\"1,20\",\"2.296\",\"76,94\",65,\"0,00\"\nAtivado,\"\"\"vinellu\"\"\",Correspondência de frase,Marca Vinellu,Vinellu marca,Não qualificado,raramente exibido; baixa qualidade,\"0,60\",0,\"0,00\",0,\"0,00\"\n";

fn terms() -> String {
    let mut s = String::from(
        "Relatório de termos de pesquisa\n3 de outubro de 2026 - 5 de outubro de 2026\nTermo de pesquisa,Tipo de corresp.,Adicionada/excluída,Campanha,Grupo de anúncios,Cliques,Impr.,Custo,Conversões\n",
    );
    s.push_str("malbec,Correspondência exata,Nenhum,\"Catálogo: uvas\",Uvas tintas,4,74,\"4,74\",\"0,00\"\n");
    s.push_str("vagas emprego vinho,Correspondência de frase,Nenhum,\"Catálogo: uvas\",Uvas tintas,1,3,\"1,10\",\"0,00\"\n");
    s.push_str("vinho chato,Correspondência de frase,Excluída,\"Catálogo: uvas\",Uvas tintas,1,2,\"1,00\",\"0,00\"\n");
    for i in 0..35 {
        s.push_str(&format!(
            "malbec rotulo {i},Correspondência de frase,Nenhum,\"Catálogo: uvas\",Uvas tintas,1,5,\"0,{:02}\",\"0,00\"\n",
            i + 10
        ));
    }
    s
}

fn digest_all() -> Performance {
    let files: Vec<ReportInput> = vec![
        ("campanha.csv".into(), read_table(CAMPAIGNS.as_bytes())),
        ("palavras.csv".into(), read_table(KEYWORDS.as_bytes())),
        ("termos.csv".into(), read_table(terms().as_bytes())),
        ("notas.csv".into(), read_table(b"foo,bar\n1,2\n")),
    ];
    digest(&files, &account())
}

#[test]
fn campaigns_follow_the_run_and_others_are_ignored() {
    let p = digest_all();
    assert_eq!(p.window.as_ref().map(|w| w.days), Some(3));
    assert_eq!(p.reports.len(), 3);
    assert_eq!(p.unknown_files.len(), 1);
    assert!(p.unknown_files[0].starts_with("notas.csv: "));
    assert_eq!(p.ignored_campaigns, ["Rotulos antigo"]);
    let names: Vec<&str> = p.campaigns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Catálogo: uvas", "Marca Vinellu"]);

    let c = p.campaign("catálogo: UVAS").unwrap();
    assert_eq!(c.live_status, Some(LiveStatus::Enabled));
    assert!(c.status_reasons.contains("política"));
    assert_eq!(c.budget, Some(70.0));
    assert_eq!(c.metrics.clicks, 156);
    assert_eq!(c.metrics.impressions, 5874);
    assert_eq!(c.metrics.avg_cpc, Some(1.15));
    assert!(c.thin, "3 days is too little to judge");
}

#[test]
fn keywords_carry_their_signals() {
    let p = digest_all();
    let k = &p.campaigns[0].ad_groups[0].keywords[0];
    assert_eq!(
        (k.text.as_str(), k.match_type),
        ("malbec", Some(MatchType::Phrase))
    );
    assert_eq!(k.signals, [Signal::BelowFirstPage]);
    assert_eq!(k.max_cpc, Some(1.2));
    assert_eq!(k.metrics.cost, 76.94);
    let brand = &p.campaigns[1].ad_groups[0].keywords[0];
    assert_eq!(brand.signals, [Signal::RarelyShown, Signal::LowQuality]);
}

#[test]
fn search_terms_are_marked_cut_and_summed() {
    let p = digest_all();
    let c = &p.campaigns[0];
    let g = &c.ad_groups[0];
    assert_eq!(g.search_terms_total, 38);
    assert_eq!(g.search_terms.len(), TERMS_PER_GROUP);
    assert_eq!(g.search_terms[0].text, "malbec", "most expensive first");
    let state = |t: &str| g.search_terms.iter().find(|x| x.text == t).map(|x| x.state);
    assert_eq!(state("malbec"), Some(TermState::Keyword));
    assert_eq!(state("vagas emprego vinho"), Some(TermState::Negative));
    assert_eq!(state("vinho chato"), Some(TermState::Excluded));
    assert_eq!(state("malbec rotulo 34"), Some(TermState::New));
    let listed: f64 = 4.74 + 1.10 + 1.00 + (10..45).map(|c| c as f64 / 100.0).sum::<f64>();
    assert!((g.cost_without_conversions - listed).abs() < 1e-6);
    let hidden = c.hidden_terms_cost.unwrap();
    assert!((hidden - (178.93 - listed)).abs() < 0.01, "{hidden}");
}

#[test]
fn without_a_campaign_report_totals_come_from_keywords() {
    let files: Vec<ReportInput> = vec![("k.csv".into(), read_table(KEYWORDS.as_bytes()))];
    let p = digest(&files, &account());
    assert_eq!(p.campaigns[0].metrics.clicks, 65);
    assert_eq!(p.campaigns[0].hidden_terms_cost, None);
    assert_eq!(p.campaigns[0].live_status, None);
}

#[test]
fn enough_days_and_clicks_is_not_thin() {
    let csv = CAMPAIGNS
        .replace(
            "3 de outubro de 2026 - 5 de outubro de 2026",
            "1 de setembro de 2026 - 30 de setembro de 2026",
        )
        .replace(",156\n", ",1560\n");
    let files: Vec<ReportInput> = vec![("c.csv".into(), read_table(csv.as_bytes()))];
    let p = digest(&files, &account());
    assert_eq!(p.window.as_ref().map(|w| w.days), Some(30));
    assert!(!p.campaigns[0].thin);
    let brand = &p.campaigns[1];
    assert!(
        !brand.thin && brand.stalled,
        "a month without impressions is broken, not thin"
    );
}

const LOST: &str = "Parc. impr. perdidas na rede de pesquisa (orçamento)";

fn by_day(rows: &[(&str, &str)]) -> String {
    let mut s = format!(
        "Campanha por dia\n3 de outubro de 2026 - 5 de outubro de 2026\nCampanha,Dia,Cliques,{LOST}\n"
    );
    for (day, lost) in rows {
        s.push_str(&format!("Marca Vinellu,{day},1,\"{lost}%\"\n"));
    }
    s
}

#[test]
fn campaign_report_lost_share_wins_over_day_rows_in_any_file_order() {
    let campaigns = format!(
        "Relatório\n3 de outubro de 2026 - 5 de outubro de 2026\nCampanha,Cliques,{LOST}\nMarca Vinellu,3,\"40,00%\"\n"
    );
    let days = by_day(&[("2026-10-03", "10,00"), ("2026-10-05", "0,00")]);
    for files in [
        vec![("a.csv", campaigns.clone()), ("b.csv", days.clone())],
        vec![("a.csv", days.clone()), ("b.csv", campaigns.clone())],
    ] {
        let inputs: Vec<ReportInput> = files
            .iter()
            .map(|(n, t)| (n.to_string(), read_table(t.as_bytes())))
            .collect();
        let p = digest(&inputs, &account());
        assert_eq!(p.campaigns[1].lost_to_budget_pct, Some(40.0));
    }
}

#[test]
fn day_rows_alone_average_the_lost_share() {
    let days = by_day(&[
        ("2026-10-03", "10,00"),
        ("2026-10-04", "20,00"),
        ("2026-10-05", "0,00"),
    ]);
    let p = digest(&[("d.csv".into(), read_table(days.as_bytes()))], &account());
    assert_eq!(p.campaigns[1].lost_to_budget_pct, Some(10.0));
    assert_eq!(p.campaigns[1].metrics.clicks, 3);
}

#[test]
fn reports_of_different_periods_are_flagged_and_hidden_cost_is_not_guessed() {
    let terms = terms().replace(
        "3 de outubro de 2026 - 5 de outubro de 2026",
        "1 de outubro de 2026 - 5 de outubro de 2026",
    );
    let files: Vec<ReportInput> = vec![
        ("c.csv".into(), read_table(CAMPAIGNS.as_bytes())),
        ("t.csv".into(), read_table(terms.as_bytes())),
    ];
    let p = digest(&files, &account());
    assert!(p.mixed_windows);
    assert_eq!(p.campaigns[0].hidden_terms_cost, None);
    let windows: Vec<u32> = p
        .reports
        .iter()
        .filter_map(|r| r.window.as_ref().map(|w| w.days))
        .collect();
    assert_eq!(windows, [3, 5]);
    assert!(!digest_all().mixed_windows);
}

#[test]
fn assets_report_gives_each_text_its_label() {
    let csv = "Relatório de recursos\n3 de outubro de 2026 - 5 de outubro de 2026\nRecurso,Tipo de recurso,Campanha,Grupo de anúncios,Classificação de desempenho,Impr.\nDescubra seu vinho,Título,\"Catálogo: uvas\",Uvas tintas,Melhor,\"1.200\"\nTexto fraco,Descrição,\"Catálogo: uvas\",Uvas tintas,Baixo,40\n";
    let p = digest(&[("a.csv".into(), read_table(csv.as_bytes()))], &account());
    let assets = &p.campaigns[0].ad_groups[0].assets;
    assert_eq!(assets.len(), 2);
    assert_eq!(
        (
            assets[0].text.as_str(),
            assets[0].kind.as_str(),
            assets[0].label.as_str(),
            assets[0].impressions
        ),
        ("Descubra seu vinho", "Título", "Melhor", 1200)
    );
    assert_eq!(assets[1].label, "Baixo");
}

/// The by-day campaign report Google exports with `Segment > Day`: one row per campaign per day.
fn campaigns_by_day(rows: &[(&str, &str, &str, &str, &str, &str, &str)]) -> String {
    let mut s = String::from(
        "Relatório de campanha\n25 de setembro de 2026 - 9 de outubro de 2026\nDia,Status da campanha,Campanha,Orçamento,Status,Motivos do status,Custo,Conversões,Impr.,Cliques\n",
    );
    for (day, campaign, budget, reasons, cost, conversions, impressions) in rows {
        let clicks = if *impressions == "0" { "0" } else { "40" };
        s.push_str(&format!(
            "{day},Ativada,\"{campaign}\",\"{budget}\",Qualificado,{reasons},\"{cost}\",\"{conversions}\",{impressions},{clicks}\n"
        ));
    }
    s
}

fn live_by_day() -> Performance {
    let mut rows = Vec::new();
    for day in ["2026-09-25", "2026-10-01"] {
        rows.push((day, "Catálogo: uvas", "70,00", "", "0,00", "0,00", "0"));
        rows.push((
            day,
            "Marca Vinellu",
            "25,00",
            "Nenhum anúncio",
            "0,00",
            "0,00",
            "0",
        ));
    }
    for day in ["2026-10-02", "2026-10-05", "2026-10-09"] {
        rows.push((day, "Catálogo: uvas", "70,00", "", "30,00", "0,00", "900"));
    }
    rows.push((
        "2026-10-09",
        "Marca Vinellu",
        "25,00",
        "Nenhum anúncio",
        "0,00",
        "0,00",
        "0",
    ));
    let csv = campaigns_by_day(&rows);
    digest(&[("d.csv".into(), read_table(csv.as_bytes()))], &account())
}

#[test]
fn day_rows_alone_carry_the_budget_and_status_of_the_last_day() {
    let p = live_by_day();
    let brand = p.campaign("Marca Vinellu").unwrap();
    assert_eq!(brand.budget, Some(25.0));
    assert_eq!(brand.live_status, Some(LiveStatus::Enabled));
    assert_eq!(brand.status_reasons, "Nenhum anúncio");
}

#[test]
fn daily_cost_counts_the_days_since_the_first_impression() {
    let p = live_by_day();
    let c = p.campaign("Catálogo: uvas").unwrap();
    assert_eq!(c.days_running, Some(8), "2 to 9 October");
    assert_eq!(c.avg_daily_cost, Some(11.25), "90 over 8 days");
    assert_eq!(c.budget_use_pct, Some(16.07));
}

#[test]
fn no_impressions_for_a_week_or_no_ads_is_stalled_not_thin() {
    let p = live_by_day();
    let brand = p.campaign("Marca Vinellu").unwrap();
    assert!(brand.stalled);
    assert!(!brand.thin, "a broken campaign is not waiting for data");
    assert!(!p.campaign("Catálogo: uvas").unwrap().stalled);

    // A campaign report with a status reason of no ads is stalled whatever the days.
    let csv = CAMPAIGNS.replace("Qualificado,,", "Não qualificado,Nenhum anúncio,");
    let p = digest(&[("c.csv".into(), read_table(csv.as_bytes()))], &account());
    assert!(p.campaigns[1].stalled);
}

#[test]
fn clicks_without_conversions_while_another_campaign_converts_is_untracked() {
    let csv = CAMPAIGNS.replace(
        "Marca Vinellu,\"25,00\",Qualificado,,\"0,00\",\"0,00\",0,0",
        "Marca Vinellu,\"25,00\",Qualificado,,\"80,00\",\"40,00\",900,120",
    );
    let p = digest(&[("c.csv".into(), read_table(csv.as_bytes()))], &account());
    assert!(p.campaign("Catálogo: uvas").unwrap().untracked);
    assert!(!p.campaign("Marca Vinellu").unwrap().untracked);
    assert!(
        !p.campaign("Marca Vinellu").unwrap().thin,
        "40 conversions are enough to judge in 3 days"
    );

    // Nothing converts in the account: no campaign is singled out.
    assert!(!digest_all().campaigns[0].untracked);
}

#[test]
fn idle_budget_is_what_enabled_campaigns_leave_unspent() {
    let p = live_by_day();
    // Catálogo spends 11.25 of 70, Marca 0 of 25.
    assert_eq!(p.idle_budget, Some(83.75));
    assert_eq!(
        digest_all().idle_budget,
        Some(35.36),
        "3 days: 70 - 59.64 and 25 - 0"
    );
}

#[test]
fn missing_reports_are_named() {
    let p = live_by_day();
    assert_eq!(
        p.missing_reports,
        [
            ReportKind::Keywords,
            ReportKind::SearchTerms,
            ReportKind::Assets
        ]
    );
    assert_eq!(digest_all().missing_reports, [ReportKind::Assets]);
}

#[test]
fn findings_name_what_needs_attention() {
    let f = live_by_day().findings();
    assert!(
        f.iter()
            .any(|l| l.starts_with("stalled") && l.contains("Marca Vinellu")),
        "{f:?}"
    );
    assert!(
        f.iter().any(|l| l == "budget unspent every day: 83.75"),
        "{f:?}"
    );
    assert!(
        f.iter().any(|l| l.contains("Keywords, SearchTerms")),
        "{f:?}"
    );
    assert!(
        digest_all()
            .findings()
            .iter()
            .all(|l| !l.starts_with("missing"))
    );
}
