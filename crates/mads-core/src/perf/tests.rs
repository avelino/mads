use super::*;
use crate::{
    google::{
        Account, AdGroup, BidStrategy, Campaign, CampaignKind, Cents, Intent, Keyword, MatchType,
        Rsa,
    },
    input::AppStore,
};

fn kw(text: &str, match_type: MatchType) -> Keyword {
    Keyword {
        text: text.into(),
        match_type,
    }
}

fn campaign(name: &str, groups: &[(&str, Vec<Keyword>)]) -> Campaign {
    Campaign {
        app_store: None,
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
        ("malbec", Some(ReportedMatch::Phrase))
    );
    assert_eq!(k.signals, [Signal::BelowFirstPage]);
    assert_eq!(k.max_cpc, Some(1.2));
    assert_eq!(k.metrics.cost, 76.94);
    let brand = &p.campaigns[1].ad_groups[0].keywords[0];
    assert_eq!(brand.signals, [Signal::RarelyShown, Signal::LowQuality]);
}

#[test]
fn keywords_keep_broad_and_the_status_of_the_keyword_itself() {
    let text = "Relatório de palavras-chave\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da palavra-chave,Palavra-chave,Tipo de corresp.,Campanha,Grupo de anúncios,Status,Cliques\nAtivado,malbec,Correspondência ampla,\"Catálogo: uvas\",Uvas tintas,Limitado,3\nPausado,\"\"\"malbec\"\"\",Correspondência de frase,\"Catálogo: uvas\",Uvas tintas,Qualificado,2\nRemovido,[malbec],Correspondência exata,\"Catálogo: uvas\",Uvas tintas,Qualificado,1\nAtivado,syrah,Correspondência modificada,\"Catálogo: uvas\",Uvas tintas,Qualificado,1\n";
    let files: Vec<ReportInput> = vec![("palavras.csv".into(), read_table(text.as_bytes()))];
    let p = digest(&files, &account());
    let seen: Vec<_> = p.campaigns[0].ad_groups[0]
        .keywords
        .iter()
        .map(|k| (k.text.as_str(), k.match_type, k.live_status))
        .collect();
    assert_eq!(
        seen,
        [
            ("malbec", Some(ReportedMatch::Broad), Some(LiveStatus::Enabled)),
            ("malbec", Some(ReportedMatch::Phrase), Some(LiveStatus::Paused)),
            ("malbec", Some(ReportedMatch::Exact), Some(LiveStatus::Removed)),
            ("syrah", None, Some(LiveStatus::Enabled)),
        ],
        "the serving status in the Status column is not the keyword status"
    );
    assert!(p.reports.iter().any(|r| r.kind == ReportKind::Keywords));
}

#[test]
fn a_keywords_report_without_a_status_column_leaves_the_status_empty() {
    let text = "Keyword report\nKeyword,Match type,Campaign,Ad group,Clicks\nmalbec,Broad match,\"Catálogo: uvas\",Uvas tintas,3\n";
    let files: Vec<ReportInput> = vec![("kw.csv".into(), read_table(text.as_bytes()))];
    let p = digest(&files, &account());
    let k = &p.campaigns[0].ad_groups[0].keywords[0];
    assert_eq!((k.match_type, k.live_status), (Some(ReportedMatch::Broad), None));
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

const AD_GROUPS: &str = "Relatório do grupo de anúncios\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus do grupo de anúncios,Grupo de anúncios,Campanha,Código da moeda,CPC máx. padrão,Impr.,Custo,Cliques,Conversões,Valor conv.\nAtivado,Uvas tintas,\"Catálogo: uvas\",BRL,\"1,20\",\"2.300\",\"80,00\",70,\"2,00\",\"0,00\"\nAtivado,Grupo antigo,Rotulos antigo,BRL,\"1,00\",10,\"1,00\",1,\"0,00\",\"0,00\"\nTotal: Pesquisa,,,BRL,,\"2.310\",\"81,00\",71,\"2,00\",\"0,00\"\n";

#[test]
fn an_ad_group_report_gives_group_totals_and_no_keywords() {
    let files: Vec<ReportInput> = vec![("g.csv".into(), read_table(AD_GROUPS.as_bytes()))];
    let p = digest(&files, &account());
    assert_eq!(p.reports[0].kind, ReportKind::AdGroups);
    assert_eq!(p.ignored_campaigns, ["Rotulos antigo"]);
    let g = &p.campaigns[0].ad_groups[0];
    assert_eq!(g.name, "Uvas tintas");
    let m = g.metrics.as_ref().unwrap();
    assert_eq!((m.impressions, m.clicks, m.cost), (2300, 70, 80.0));
    assert_eq!(m.conversions, 2.0);
    assert!(g.keywords.is_empty() && g.search_terms.is_empty());
    assert_eq!(
        p.campaigns[0].metrics.clicks, 70,
        "without a campaign report, totals come from the groups"
    );
    assert_eq!(
        p.blind_reports(),
        ["Keywords", "SearchTerms"],
        "group totals say nothing about keywords"
    );
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

const STRATEGY: &str = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Orçamento,Tipo de estratégia de lances,Custo,Conversões,Impr.,Cliques\nAtivada,\"Catálogo: uvas\",\"70,00\",Maximizar as conversões,\"10,00\",\"0,00\",100,10\nAtivada,Marca Vinellu,\"25,00\",CPC manual,\"0,00\",\"0,00\",0,0\n";

#[test]
fn the_campaign_report_says_how_each_campaign_bids_today() {
    let files: Vec<ReportInput> = vec![("c.csv".into(), read_table(STRATEGY.as_bytes()))];
    let p = digest(&files, &account());
    let (catalog, brand) = (&p.campaigns[0], &p.campaigns[1]);
    assert_eq!(catalog.bid_strategy, "Maximizar as conversões");
    assert!(catalog.bids_unlike("Manual CPC"), "changed by hand");
    assert!(!catalog.bids_unlike("Maximize conversions"));
    assert!(!brand.bids_unlike("Manual CPC"));
    assert!(brand.bids_unlike("Maximize clicks"));
}

#[test]
fn a_report_without_the_strategy_column_says_nothing_about_bidding() {
    let files: Vec<ReportInput> = vec![("c.csv".into(), read_table(CAMPAIGNS.as_bytes()))];
    let p = digest(&files, &account());
    assert_eq!(p.campaigns[0].bid_strategy, "");
    assert!(!p.campaigns[0].bids_unlike("Manual CPC"));
}

#[test]
fn value_bidding_is_not_conversion_bidding() {
    let c = CampaignPerf {
        bid_strategy: "Maximizar o valor da conversão".into(),
        ..Default::default()
    };
    assert!(c.bids_unlike("Maximize conversions"));
}

const GROUP_STATUS: &str = "Relatório do grupo de anúncios\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus do grupo de anúncios,Grupo de anúncios,Campanha,Impr.,Custo,Cliques,Conversões\nAtivado,Uvas tintas,\"Catálogo: uvas\",10,\"1,00\",1,\"0,00\"\nPausado,Uvas paradas,\"Catálogo: uvas\",0,\"0,00\",0,\"0,00\"\nRemovido,Uvas mortas,\"Catálogo: uvas\",0,\"0,00\",0,\"0,00\"\n";

#[test]
fn the_ad_group_report_gives_the_status_of_each_group() {
    let files: Vec<ReportInput> = vec![("g.csv".into(), read_table(GROUP_STATUS.as_bytes()))];
    let p = digest(&files, &account());
    let status: Vec<Option<LiveStatus>> = p.campaigns[0]
        .ad_groups
        .iter()
        .map(|g| g.live_status)
        .collect();
    assert_eq!(
        status,
        [
            Some(LiveStatus::Enabled),
            Some(LiveStatus::Paused),
            Some(LiveStatus::Removed)
        ]
    );
}

const ASSET_GROUPS: &str = "Relatório de grupos de recursos\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus do grupo de recursos,Grupo de recursos,Campanha,Impr.,Custo,Conversões\nENABLED,Tintos,\"Catálogo: uvas\",100,\"5,00\",\"1,00\"\nENABLED,Brancos,\"Catálogo: uvas\",50,\"2,00\",\"0,00\"\n";

#[test]
fn an_asset_group_report_gives_group_totals_not_campaign_totals() {
    let files: Vec<ReportInput> = vec![("a.csv".into(), read_table(ASSET_GROUPS.as_bytes()))];
    let p = digest(&files, &account());
    assert_eq!(p.reports[0].kind, ReportKind::AdGroups);
    let c = &p.campaigns[0];
    let names: Vec<&str> = c.ad_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Tintos", "Brancos"]);
    assert_eq!(c.ad_groups[0].live_status, Some(LiveStatus::Enabled));
    assert_eq!(
        c.metrics.cost, 7.0,
        "the sum of the groups, not the last row"
    );
    assert_eq!(p.missing_reports[0], ReportKind::Campaigns);
}

/// An Android App campaign, an iOS one and a Search campaign: an account whose goal is installs.
fn app_account() -> Account {
    let app = |name: &str, store| {
        let mut c = campaign(name, &[]);
        c.kind = CampaignKind::AppInstalls;
        c.app_store = Some(store);
        c
    };
    Account {
        brand_kit: None,
        campaigns: vec![
            app("App Android", AppStore::GooglePlay),
            app("App iOS", AppStore::AppStore),
            campaign("Busca", &[]),
        ],
    }
}

const WINDOW_PT: &str = "1 de outubro de 2026 - 30 de outubro de 2026";

fn app_campaigns(ios_clicks: u32) -> String {
    format!(
        "Relatório de campanha\n{WINDOW_PT}\nCampanha,Orçamento,Custo,Conversões,Impr.,Cliques\nApp Android,\"50,00\",\"300,00\",\"400,00\",\"20.000\",900\nApp iOS,\"50,00\",\"200,00\",\"0,00\",\"15.000\",{ios_clicks}\nBusca,\"20,00\",\"100,00\",\"30,00\",\"5.000\",300\n"
    )
}

const ACTIONS_PT: &str = "Relatório de campanha\n1 de outubro de 2026 - 30 de outubro de 2026\nCampanha,Ação de conversão,Categoria de conversão,Conversões,Todas as conv.\nApp Android,App (Android) primeira abertura,Download,\"400,00\",\"410,00\"\nApp iOS,--,--,\"0,00\",\"0,00\"\nBusca,Compra no site,Compra,\"20,00\",\"20,00\"\nBusca,Contato,Contato,\"10,00\",\"12,00\"\nBusca,App (Android) primeira abertura,Download,\"0,00\",\"1,00\"\nTotal: Conta,,,\"430,00\",\"443,00\"\n";

fn digest_apps(campaigns: &str, actions: Option<&str>, account: &Account) -> Performance {
    let mut files: Vec<ReportInput> = vec![("c.csv".into(), read_table(campaigns.as_bytes()))];
    if let Some(actions) = actions {
        files.push(("a.csv".into(), read_table(actions.as_bytes())));
    }
    digest(&files, account)
}

#[test]
fn each_campaign_lists_the_conversion_actions_it_counted_most_first() {
    let p = digest_apps(&app_campaigns(600), Some(ACTIONS_PT), &app_account());
    assert_eq!(p.reports[1].kind, ReportKind::ConversionActions);
    let busca = p.campaign("Busca").unwrap();
    let names: Vec<&str> = busca
        .conversion_actions
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Compra no site",
            "Contato",
            "App (Android) primeira abertura"
        ]
    );
    assert_eq!(busca.conversion_actions[1].conversions, 10.0);
    assert_eq!(busca.conversion_actions[1].all_conversions, Some(12.0));
    assert_eq!(busca.conversion_actions[1].category, "Contato");
    assert!(
        p.campaign("App iOS").unwrap().conversion_actions.is_empty(),
        "-- is no action"
    );
    assert_eq!(p.campaigns[0].metrics.conversions, 400.0, "totals stay");
}

#[test]
fn the_conversion_actions_kept_per_campaign_are_capped() {
    let mut csv = String::from(
        "Campaign report\nOct 1, 2026 - Oct 30, 2026\nCampaign,Conversion action,Conversions\n",
    );
    for i in 0..8 {
        csv.push_str(&format!("Busca,Action {i},{}\n", i + 1));
    }
    let p = digest_apps(&app_campaigns(600), Some(&csv), &app_account());
    let kept = &p.campaign("Busca").unwrap().conversion_actions;
    assert_eq!(kept.len(), crate::perf::conversions::ACTIONS_PER_CAMPAIGN);
    assert_eq!(kept[0].name, "Action 7", "the most conversions stay");
}

#[test]
fn an_app_campaign_with_no_action_for_its_store_is_untracked_with_the_reason() {
    let p = digest_apps(&app_campaigns(600), Some(ACTIONS_PT), &app_account());
    let ios = p.campaign("App iOS").unwrap();
    assert!(ios.untracked);
    assert_eq!(ios.untracked_reason.as_deref(), Some(NO_ACTION_FOR_STORE));
    let android = p.campaign("App Android").unwrap();
    assert!(!android.untracked && android.untracked_reason.is_none());
    assert!(!p.campaign("Busca").unwrap().untracked);

    let f = p.findings();
    assert!(
        f.iter().any(|l| {
            l.starts_with("untracked, no conversion action for this store")
                && l.ends_with("App iOS")
        }),
        "{f:?}"
    );
    assert!(
        f.iter()
            .all(|l| !(l.starts_with("clicks without conversions") && l.contains("App iOS"))),
        "the generic line does not repeat it: {f:?}"
    );
    assert!(
        f.iter().any(|l| l.starts_with(
            "conversion actions of App Android: App (Android) primeira abertura (400)"
        )),
        "{f:?}"
    );
}

#[test]
fn an_action_that_names_the_other_store_does_not_count() {
    let csv = ACTIONS_PT.replace(
        "App iOS,--,--,\"0,00\",\"0,00\"",
        "App iOS,App (Android) primeira abertura,Download,\"5,00\",\"5,00\"",
    );
    let p = digest_apps(&app_campaigns(600), Some(&csv), &app_account());
    assert_eq!(
        p.campaign("App iOS").unwrap().untracked_reason.as_deref(),
        Some(NO_ACTION_FOR_STORE)
    );
    let ok = csv.replace("App iOS,App (Android)", "App iOS,App (iOS)");
    let p = digest_apps(&app_campaigns(600), Some(&ok), &app_account());
    assert_eq!(p.campaign("App iOS").unwrap().untracked_reason, None);
}

#[test]
fn clicks_not_enough_to_judge_or_no_conversion_report_leave_the_reason_out() {
    let few = digest_apps(&app_campaigns(50), Some(ACTIONS_PT), &app_account());
    assert_eq!(few.campaign("App iOS").unwrap().untracked_reason, None);

    let without = digest_apps(&app_campaigns(600), None, &app_account());
    let ios = without.campaign("App iOS").unwrap();
    assert!(ios.untracked, "the generic rule still applies");
    assert_eq!(ios.untracked_reason, None);
    assert!(
        without
            .findings()
            .iter()
            .all(|l| !l.contains("for this store"))
    );
}

#[test]
fn an_app_campaign_without_any_action_is_flagged_even_when_nothing_converts() {
    let nothing = app_campaigns(600).replace("\"400,00\"", "\"0,00\"");
    let actions = "Campaign report\nOct 1, 2026 - Oct 30, 2026\nCampaign,Conversion action,Conversions\nBusca,Contact,0.00\n";
    let p = digest_apps(&nothing, Some(actions), &app_account());
    for name in ["App Android", "App iOS"] {
        let c = p.campaign(name).unwrap();
        assert_eq!(
            c.untracked_reason.as_deref(),
            Some(NO_ACTION_FOR_STORE),
            "{name}"
        );
    }
}

#[test]
fn search_and_demand_gen_campaigns_counting_what_is_not_installs_are_named() {
    let p = digest_apps(&app_campaigns(600), Some(ACTIONS_PT), &app_account());
    let busca = p.campaign("Busca").unwrap();
    assert_eq!(busca.off_goal_actions, ["Compra no site", "Contato"]);
    assert!(
        p.campaign("App Android")
            .unwrap()
            .off_goal_actions
            .is_empty()
    );
    let f = p.findings();
    assert!(
        f.iter().any(|l| {
            l.starts_with("Busca optimizes on conversions that are not the goal")
                && l.ends_with("Compra no site, Contato")
        }),
        "{f:?}"
    );

    let mut account = app_account();
    account.campaigns[2].kind = CampaignKind::DemandGen;
    let p = digest_apps(&app_campaigns(600), Some(ACTIONS_PT), &account);
    assert_eq!(p.campaign("Busca").unwrap().off_goal_actions.len(), 2);

    account.campaigns[2].kind = CampaignKind::PerformanceMax;
    let p = digest_apps(&app_campaigns(600), Some(ACTIONS_PT), &account);
    assert!(p.campaign("Busca").unwrap().off_goal_actions.is_empty());
}

#[test]
fn without_an_app_campaign_the_goal_is_not_installs() {
    let site = Account {
        brand_kit: None,
        campaigns: vec![campaign("App Android", &[]), campaign("Busca", &[])],
    };
    let p = digest_apps(&app_campaigns(600), Some(ACTIONS_PT), &site);
    assert!(p.campaigns.iter().all(|c| c.off_goal_actions.is_empty()));
    assert!(p.campaigns.iter().all(|c| c.untracked_reason.is_none()));
    assert!(p.findings().iter().all(|l| !l.contains("not the goal")));
}

fn networks_pt(display_clicks: u32) -> String {
    format!(
        "Relatório de campanha\n{WINDOW_PT}\nCampanha,Rede (com parceiros de pesquisa),Impr.,Cliques,Custo,Conversões\nApp Android,Pesquisa Google,\"10.000\",400,\"150,00\",\"300,00\"\nApp Android,Rede de Display,\"5.000\",{display_clicks},\"150,00\",\"100,00\"\nTotal: Conta,,\"15.000\",1300,\"300,00\",\"400,00\"\n"
    )
}

fn digest_networks(csv: &str, account: &Account) -> Performance {
    digest(&[("n.csv".into(), read_table(csv.as_bytes()))], account)
}

#[test]
fn the_network_report_gives_each_campaign_its_shares_and_ctr() {
    let p = digest_networks(&networks_pt(750), &app_account());
    assert_eq!(p.reports[0].kind, ReportKind::Networks);
    let nets = &p.campaign("App Android").unwrap().networks;
    assert_eq!(nets.len(), 2);
    assert_eq!(
        nets[0].network,
        Network::Search,
        "a tie goes by network order"
    );
    assert_eq!(nets[0].cost_share_pct, Some(50.0));
    assert_eq!(nets[0].conversions_share_pct, Some(75.0));
    assert_eq!(nets[0].ctr_pct, Some(4.0));
    assert_eq!(nets[1].network, Network::Display);
    assert_eq!(nets[1].conversions_share_pct, Some(25.0));
    assert_eq!(nets[1].ctr_pct, Some(15.0));
    assert!(p.campaign("Busca").unwrap().networks.is_empty());
    assert_eq!(
        p.campaigns[0].metrics.clicks, 0,
        "a network report does not stand in for the campaign totals"
    );
}

#[test]
fn display_ctr_on_an_app_campaign_is_flagged_above_the_line_and_not_at_it() {
    let at = digest_networks(&networks_pt(750), &app_account());
    assert!(
        !at.campaign("App Android")
            .unwrap()
            .accidental_display_clicks
    );
    assert!(at.findings().iter().all(|l| !l.starts_with("Display CTR")));

    let above = digest_networks(&networks_pt(751), &app_account());
    assert!(
        above
            .campaign("App Android")
            .unwrap()
            .accidental_display_clicks
    );
    let f = above.findings();
    assert!(
        f.iter().any(|l| {
            l.starts_with("Display CTR of App Android is 15.02%, above 15%")
                && l.contains("do not raise its budget on cost per install alone")
        }),
        "{f:?}"
    );
    let line = "networks of App Android: search 50% of cost, 75% of conversions, CTR 4%; display 50% of cost";
    assert!(f.iter().any(|l| l.starts_with(line)), "{f:?}");
}

#[test]
fn display_ctr_is_flagged_the_same_in_an_english_report() {
    let csv = "Campaign report\nOct 1, 2026 - Oct 30, 2026\nCampaign,Network (with search partners),Impr.,Clicks,Cost,Conversions\nApp iOS,Display Network,\"5,000\",900,150.00,10.00\nApp iOS,Search Partners,\"1,000\",20,10.00,1.00\n";
    let p = digest_networks(csv, &app_account());
    let ios = p.campaign("App iOS").unwrap();
    assert!(ios.accidental_display_clicks);
    assert_eq!(ios.networks[0].network, Network::Display);
    assert_eq!(ios.networks[1].network, Network::SearchPartners);
}

#[test]
fn display_ctr_on_a_search_campaign_is_only_a_number() {
    let csv = networks_pt(900).replace("App Android", "Busca");
    let p = digest_networks(&csv, &app_account());
    let busca = p.campaign("Busca").unwrap();
    assert!(!busca.accidental_display_clicks);
    assert_eq!(busca.networks.len(), 2);
}

#[test]
fn a_single_network_gets_no_line_of_its_own() {
    let csv = "Campaign report\nOct 1, 2026 - Oct 30, 2026\nCampaign,Network,Impr.,Clicks,Cost\nBusca,Google search,\"1,000\",50,10.00\n";
    let p = digest_networks(csv, &app_account());
    assert_eq!(p.campaign("Busca").unwrap().networks.len(), 1);
    assert!(p.findings().iter().all(|l| !l.starts_with("networks of")));
}

const OUTSIDE_REPORT: &str = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Orçamento,Custo,Conversões,Impr.,Cliques\nAtivada,\"Catálogo: uvas\",\"70,00\",\"100,00\",\"5,00\",\"5.000\",200\nAtivada,Antiga sem medir,\"40,00\",\"60,00\",\"0,00\",\"3.000\",150\nPausada,Antiga pausada,\"40,00\",\"60,00\",\"0,00\",\"3.000\",150\nRemovida,Antiga removida,\"40,00\",\"60,00\",\"0,00\",\"3.000\",150\nTotal: Conta,,,,,,\n";

#[test]
fn campaigns_outside_the_run_keep_their_numbers_and_the_blind_ones_are_flagged() {
    let files = vec![("c.csv".to_string(), read_table(OUTSIDE_REPORT.as_bytes()))];
    let p = digest(&files, &account());
    assert_eq!(p.ignored_campaigns.len(), 3);
    assert_eq!(p.outside.len(), 3);
    let by = |name: &str| p.outside.iter().find(|o| o.name == name).unwrap();
    let blind = by("Antiga sem medir");
    assert!(blind.spends_blind);
    assert_eq!(blind.live_status, Some(LiveStatus::Enabled));
    assert_eq!(blind.budget, Some(40.0));
    assert_eq!(blind.metrics.clicks, 150);
    assert_eq!(blind.cost_share_pct, Some(21.43));
    assert!(!by("Antiga pausada").spends_blind);
    assert!(!by("Antiga removida").spends_blind);
    assert_eq!(p.outside_blind().len(), 1);
    assert!(
        p.findings()
            .iter()
            .any(|l| l.contains("Antiga sem medir (21.43% of the account cost, 150 clicks)")),
        "{:?}",
        p.findings()
    );
}

#[test]
fn a_campaign_outside_the_run_is_read_from_day_rows_too() {
    let csv = "Campaign report\nOct 1, 2026 - Oct 3, 2026\nDay,Campaign status,Campaign,Budget,Cost,Conversions,Impr.,Clicks\n2026-10-01,Enabled,Old,40.00,10.00,0.00,500,60\n2026-10-02,Paused,Old,40.00,10.00,0.00,500,60\n2026-10-01,Enabled,\"Catálogo: uvas\",70.00,50.00,4.00,900,80\n";
    let files = vec![("d.csv".to_string(), read_table(csv.as_bytes()))];
    let p = digest(&files, &account());
    let old = &p.outside[0];
    assert_eq!(old.name, "Old");
    assert_eq!(old.metrics.clicks, 120);
    assert_eq!(old.live_status, Some(LiveStatus::Paused), "latest day wins");
    assert!(!old.spends_blind, "paused now");
}

#[test]
fn without_a_campaigns_report_nothing_is_known_of_the_campaigns_outside() {
    let p = digest_all_without_campaigns();
    assert_eq!(p.ignored_campaigns, Vec::<String>::new());
    assert!(p.outside.is_empty());
}

fn digest_all_without_campaigns() -> Performance {
    let files = vec![("palavras.csv".to_string(), read_table(KEYWORDS.as_bytes()))];
    digest(&files, &account())
}
