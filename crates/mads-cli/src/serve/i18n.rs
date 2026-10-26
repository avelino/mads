use axum::http::HeaderMap;

use crate::serve::auth;

pub const PT_BR: &str = "pt-BR";
pub const EN: &str = "en";

/// Translation keyed by the Portuguese text. English falls back to the key.
pub fn t(lang: &str, key: &str) -> String {
    if lang != EN {
        return key.to_string();
    }
    EN_TEXT
        .iter()
        .find(|(pt, _)| *pt == key)
        .map(|(_, en)| (*en).to_string())
        .unwrap_or_else(|| key.to_string())
}

pub fn lang_of(headers: &HeaderMap) -> String {
    if let Some(value) = auth::cookie_value(headers, "mads_lang") {
        let value = value.trim();
        if value == EN {
            return EN.into();
        }
        if value == PT_BR {
            return PT_BR.into();
        }
    }
    // English is the default. Only an explicit pt-BR preference, cookie or
    // browser, switches the interface to Portuguese.
    if let Some(value) = headers.get(axum::http::header::ACCEPT_LANGUAGE)
        && let Ok(value) = value.to_str()
        && parse_accept_language(value) == PT_BR
    {
        return PT_BR.into();
    }
    EN.into()
}

/// The highest-preference `en` or `pt-BR` tag in an `Accept-Language` header,
/// ignoring q-weighting nuances beyond the order the browser sent.
fn parse_accept_language(value: &str) -> &'static str {
    for part in value.split(',') {
        let tag = part.split(';').next().unwrap_or("").trim().to_lowercase();
        if tag == "en" || tag.starts_with("en-") {
            return EN;
        }
        if tag == "pt" || tag.starts_with("pt-") {
            return PT_BR;
        }
    }
    PT_BR
}

pub fn cookie(lang: &str) -> String {
    format!("mads_lang={lang}; Path=/; SameSite=Strict; Max-Age=31536000")
}

const EN_TEXT: &[(&str, &str)] = &[
    ("configurar em Conexões", "configure in Connections"),
    (
        "Nenhum provedor configurado ainda.",
        "No provider configured yet.",
    ),
    (
        "Abra as Conexões para salvar uma chave de API e volte aqui.",
        "Open Connections to save an API key, then come back.",
    ),
    (
        "a chave foi rejeitada pelo provedor; confira em Conexões",
        "the key was rejected by the provider; check Connections",
    ),
    // Diff table field labels (crates/mads-cli/src/serve/diff.rs).
    ("CPC padrão", "Default CPC"),
    ("Caminho 1", "Path 1"),
    ("Caminho 2", "Path 2"),
    ("Descrição adicionada", "Description added"),
    ("Descrição removida", "Description removed"),
    ("Estratégia de lance", "Bid strategy"),
    ("Extensão de chamada adicionada", "Callout added"),
    ("Extensão de chamada removida", "Callout removed"),
    ("Grupo", "Group"),
    ("Grupo de assets", "Asset group"),
    ("Intenção", "Intent"),
    ("Justificativa", "Rationale"),
    ("Justificativa do CPC", "CPC rationale"),
    ("Link do site adicionado", "Sitelink added"),
    ("Link do site removido", "Sitelink removed"),
    ("Negativa adicionada", "Negative added"),
    ("Negativa removida", "Negative removed"),
    ("Nome da empresa", "Business name"),
    ("Palavra-chave adicionada", "Keyword added"),
    ("Palavra-chave removida", "Keyword removed"),
    ("Snippet adicionado", "Snippet added"),
    ("Snippet removido", "Snippet removed"),
    ("Tema de busca adicionada", "Search theme added"),
    ("Tema de busca adicionado", "Search theme added"),
    ("Tema de busca removida", "Search theme removed"),
    ("Tema de busca removido", "Search theme removed"),
    ("Título adicionado", "Headline added"),
    ("Título longo adicionado", "Long headline added"),
    ("Título longo removido", "Long headline removed"),
    ("Título removido", "Headline removed"),
    ("URL do link do site", "Sitelink URL"),
    ("URL final", "Final URL"),
    ("Valores do snippet", "Snippet values"),
    // Provider status strings (Source::label and ProviderStatus::describe).
    ("Job", "Job"),
    ("Revisar otimização", "Review optimization"),
    ("Endpoint", "Endpoint"),
    (
        "o default do provedor é preenchido e você pode editar; o openai-compat usa este endpoint",
        "the provider default is prefilled and editable; openai-compat calls this endpoint",
    ),
    (
        "configure um provedor em Conexões",
        "configure a provider in Connections",
    ),
    (
        "carrega do provedor, ou digite um id",
        "loads from the provider, or type an id",
    ),
    (
        "claude-cli, codex-cli e gemini-cli escolhem o modelo deles",
        "claude-cli, codex-cli and gemini-cli pick their own model",
    ),
    ("buscando modelos", "loading models"),
    ("Na fila", "Queued"),
    ("Renomear", "Rename"),
    (
        "O nome aparece na lista. A pasta do projeto não muda.",
        "The name shows in the list. The project folder does not change.",
    ),
    ("Salvar nome", "Save name"),
    // Chrome and navigation.
    ("Projetos", "Projects"),
    ("Conexões", "Connections"),
    ("Studio", "Studio"),
    ("Estúdio", "Studio"),
    ("Configurações", "Settings"),
    (
        "Os arquivos ficam no seu computador. Nada é enviado ao Google Ads.",
        "Files stay on your computer. Nothing is uploaded to Google Ads.",
    ),
    (
        "Um diretório por negócio. O rascunho fica no projeto, as execuções ficam em ",
        "One directory per business. The draft lives in the project, runs live in ",
    ),
    ("Novo projeto", "New project"),
    ("Nenhum projeto ainda", "No projects yet"),
    ("Criar pelo navegador", "Create in the browser"),
    (
        "Ou, no terminal, crie a pasta com um ",
        "Or create the folder with a ",
    ),
    ("Voltar", "Back"),
    ("Abrir", "Open"),
    ("Otimizar", "Optimize"),
    ("Exportar", "Export"),
    ("Rascunho", "Draft"),
    ("Nova execução", "New run"),
    ("Visualizar", "View"),
    ("Status", "Status"),
    ("Atualizado", "Updated"),
    ("Nome", "Name"),
    ("Site", "Site"),
    ("Orçamento", "Budget"),
    ("Execuções", "Runs"),
    (
        "Nenhuma execução neste projeto. Gere uma conta pelo CLI ou aguarde o fluxo do estúdio.",
        "No runs in this project yet. Generate an account from the CLI or wait for the studio flow.",
    ),
    ("Próximo passo", "Next step"),
    ("Atalhos", "Shortcuts"),
    ("Resumo", "Summary"),
    ("Data", "Date"),
    ("Tokens", "Tokens"),
    ("Custo", "Cost"),
    ("Layout", "Layout"),
    ("Missões", "Missions"),
    ("Campanhas", "Campaigns"),
    ("Grupos", "Groups"),
    ("Palavras", "Keywords"),
    ("Erros", "Errors"),
    ("Avisos", "Warnings"),
    ("Validação", "Validation"),
    ("Ver tudo", "See all"),
    ("Nenhum erro.", "No errors."),
    (
        "A conta passa nas regras que bloqueiam a exportação.",
        "The account passes the rules that block the export.",
    ),
    ("Conta", "Account"),
    ("Relatório", "Report"),
    ("Arquivos", "Files"),
    ("Eventos", "Events"),
    ("Linha do tempo", "Timeline"),
    ("Visão geral", "Overview"),
    ("Performance", "Performance"),
    ("Corrigir", "Correct"),
    ("Reexportar sem IA", "Re-export without AI"),
    ("Retomar missões", "Resume missions"),
    ("Gerar campanhas", "Generate campaigns"),
    ("Revisar e iniciar", "Review and start"),
    ("Execução", "Run"),
    ("Trabalho", "Job"),
    ("Ao vivo", "Live"),
    ("Iniciar", "Start"),
    ("Cancelar", "Cancel"),
    ("Salvar", "Save"),
    ("Salvar rascunho", "Save draft"),
    ("Salvar revisão e reexportar", "Save revision and re-export"),
    ("Parar", "Stop"),
    ("Entrar", "Sign in"),
    ("Voltar aos projetos", "Back to projects"),
    ("Não encontrado", "Not found"),
    ("Esta página não existe.", "This page does not exist."),
    ("Projeto não encontrado", "Project not found"),
    ("Execução não encontrada", "Run not found"),
    ("projeto(s)", "project(s)"),
    ("execução(ões)", "run(s)"),
    ("atualizado em", "updated"),
    ("Crie uma pasta em", "Create a folder in"),
    ("com um", "with a"),
    ("ou rode", "or run"),
    (
        "nela. O estúdio encontra o projeto ao atualizar esta página.",
        "in it. The studio finds the project when you refresh this page.",
    ),
    (
        "Ou, no terminal, crie a pasta com um",
        "Or, in a terminal, create the folder with a",
    ),
    ("Projeto", "Project"),
    ("no total", "in total"),
    ("revisão de", "revision of"),
    ("missão(ões)", "mission(s)"),
    ("tokens", "tokens"),
    ("custo", "cost"),
    (
        "erro(s) bloqueiam a exportação.",
        "error(s) block the export.",
    ),
    ("ocorrência(s)", "occurrence(s)"),
    (
        "aviso(s). Avisos não bloqueiam a exportação.",
        "warning(s). Warnings do not block the export.",
    ),
    ("Arquivos e transcrições", "Files and transcripts"),
    ("Relatório completo", "Full report"),
    ("Execução · conta", "Run · account"),
    ("Total", "Total"),
    ("marca", "brand"),
    ("catálogo", "catalog"),
    ("genérica", "generic"),
    ("concorrente", "competitor"),
    ("frase", "phrase"),
    ("exata", "exact"),
    ("gerada", "generated"),
    ("faltando", "missing"),
    ("Execução · validação", "Run · validation"),
    ("erro(s),", "error(s),"),
    (
        "aviso(s). Erros bloqueiam a exportação.",
        "warning(s). Errors block the export.",
    ),
    ("Nenhum aviso.", "No warnings."),
    ("URL", "URL"),
    ("Execução · arquivos", "Run · files"),
    ("arquivo(s) em", "file(s) in"),
    ("Nenhum arquivo", "No files"),
    (
        "Uma execução incompleta ou falha não escreve arquivos em",
        "An incomplete or failed run writes no files in",
    ),
    ("Prévia", "Preview"),
    ("Execução · eventos", "Run · events"),
    ("Cada evento de", "Every event from"),
    (
        "Atualiza ao vivo enquanto o servidor tem o job.",
        "Updates live while the server holds the job.",
    ),
    ("Nenhum evento registrado.", "No events recorded."),
    (
        "Provedores de texto e de imagem. As chaves ficam no servidor, nunca no navegador.",
        "Text and image providers. Keys stay on the server, never in the browser.",
    ),
    ("status do processo", "process status"),
    ("Provedor", "Provider"),
    ("Remover", "Remove"),
    ("Estudar o site", "Study the site"),
    (
        "Nenhuma execução neste projeto. Gere a primeira conta em Nova execução.",
        "No runs in this project yet. Generate the first account from New run.",
    ),
    (
        "O rascunho precisa de atenção. Estude o site de novo ou corrija os arquivos.",
        "The draft needs attention. Study the site again or fix the files.",
    ),
    (
        "Revise o rascunho e gere a conta. Cada execução guarda um snapshot.",
        "Review the draft and generate the account. Each run keeps a snapshot.",
    ),
    ("Revisar rascunho", "Review draft"),
    ("Corrija e reexporte. Abra", "Fix and re-export. Open"),
    (
        "para editar a conta e criar uma revisão.",
        "to edit the account and create a revision.",
    ),
    (
        "Corrija e reexporte. Abra <strong>Corrigir</strong> para editar a conta e criar uma revisão.",
        "Fix and re-export. Open Correct to edit the account and create a revision.",
    ),
    ("Alterações", "Changes"),
    ("Ativos", "Assets"),
    ("Sitelinks", "Sitelinks"),
    ("Callouts", "Callouts"),
    ("Snippets", "Snippets"),
    (
        "Sitelinks, callouts e snippets são validados, mas não exportados nesta versão.",
        "Sitelinks, callouts and snippets are validated but not exported in this version.",
    ),
    ("Logo", "Logo"),
    (
        "Envie um PNG ou JPEG quadrado. O mads converte para o tamanho que o Google aceita.",
        "Upload a square PNG or JPEG. mads converts it to the size Google accepts.",
    ),
    ("Salvar logo", "Save logo"),
    ("Pesquisa", "Research"),
    (
        "research.md vai para o prompt do plano. O limite é 20.000 caracteres.",
        "research.md goes into the plan prompt. The limit is 20,000 characters.",
    ),
    ("Salvar pesquisa", "Save research"),
    ("Design", "Design"),
    (
        "DESIGN.md descreve cores, fontes e tom de voz para as campanhas de imagem. O limite é 8.000 caracteres.",
        "DESIGN.md describes colors, fonts and voice for image campaigns. The limit is 8,000 characters.",
    ),
    ("Salvar design", "Save design"),
    ("Foco", "Focus"),
    ("Nome do foco", "Focus name"),
    ("App store", "App store"),
    ("URLs do foco (uma por linha)", "Focus URLs (one per line)"),
    (
        "Termos do foco (um grupo por linha, termos por vírgula)",
        "Focus terms (one group per line, terms separated by commas)",
    ),
    ("Categoria", "Category"),
    ("Notas", "Notes"),
    ("Desfazer revisão", "Revert revision"),
    ("Configurar chave", "Configure key"),
    ("Jobs", "Jobs"),
    (
        "Execuções em andamento e as últimas que este servidor iniciou. O histórico fica na pasta de cada projeto.",
        "Running jobs and the latest ones this server started. History lives in each project folder.",
    ),
    (
        "Nenhum job iniciado desde que este servidor abriu.",
        "No jobs started since this server opened.",
    ),
    ("Nesta página", "On this page"),
    ("Dados", "Data"),
    ("Restrições", "Restrictions"),
    ("Formatos", "Formats"),
    ("Avançado", "Advanced"),
    (
        "Foco, app e Google Ads (opcional)",
        "Focus, app and Google Ads (optional)",
    ),
    ("business.toml", "business.toml"),
    // Status labels.
    ("Pronto", "Ready"),
    ("Inválido", "Invalid"),
    ("Incompleto", "Incomplete"),
    ("Cancelado", "Cancelled"),
    ("Plano pronto", "Plan ready"),
    ("Preparado", "Prepared"),
    ("Em execução", "Running"),
    ("Interrompido", "Interrupted"),
    ("Parando", "Stopping"),
    ("Concluído", "Done"),
    ("Falhou", "Failed"),
    // Common labels.
    ("Por que", "Why"),
    ("Anúncio (merge real)", "Ad (real merge)"),
    ("Negativas", "Negatives"),
    ("Negativas da campanha", "Campaign negatives"),
    ("Palavras-chave", "Keywords"),
    ("Títulos", "Headlines"),
    ("Títulos longos", "Long headlines"),
    ("Descrições", "Descriptions"),
    ("Temas de busca", "Search themes"),
    ("Imagens", "Images"),
    ("Grupos de assets", "Asset groups"),
    ("Brand kit", "Brand kit"),
    ("Correspondência", "Match type"),
    ("Arquivo de chaves", "Key file"),
    ("Chaves salvas", "Saved keys"),
    ("Provedores de texto", "Text providers"),
    ("Provedores de imagem", "Image providers"),
    ("Chave", "Key"),
    ("Tipo", "Type"),
    ("Origem", "Source"),
    ("não precisa de chave", "no key needed"),
    ("variável de ambiente", "environment variable"),
    ("arquivo 0600", "file 0600"),
    ("ausente", "missing"),
    ("não precisa", "not needed"),
    ("Verificação de URL", "URL check"),
    ("Como corrigir", "How to fix"),
    // Project admin, import and manual creation.
    (
        ", mas a pasta dessa execução não está neste projeto. Nada a comparar.",
        ", but that run's folder is not in this project. Nothing to compare.",
    ),
    (
        ", mas essa execução não está no projeto.",
        ", but that run is not in the project.",
    ),
    (
        ". Rode de novo com os dados abaixo. Isto gasta tokens.",
        ". Run it again with the data below. This spends tokens.",
    ),
    (
        ". Valores na moeda do projeto.",
        ". Values in the project currency.",
    ),
    (
        ". Você pode recuperar tudo pelo terminal.",
        ". You can recover everything from the terminal.",
    ),
    (". Você revisa depois.", ". You review it afterwards."),
    ("/dia", "/day"),
    (
        "A conta é idêntica à execução",
        "The account is identical to run",
    ),
    (
        "A conta é validada antes de gravar. Se algo quebrar, nada é salvo.",
        "The account is validated before saving. If anything breaks, nothing is saved.",
    ),
    (
        "A edição cria uma revisão. A execução original fica intacta e a reexportação não chama a IA.",
        "Editing creates a revision. The original run stays intact and re-exporting does not call the AI.",
    ),
    ("A execução falhou.", "The run failed."),
    (
        "A importação é só pelo Google Ads Editor, nunca pelos arquivos 1–5.",
        "Import is only through Google Ads Editor, never through files 1–5.",
    ),
    (
        "A otimização precisa de uma execução com todas as missões terminadas.",
        "Optimization needs a run with every mission finished.",
    ),
    (
        "A pasta precisa existir na mesma máquina que roda o mads studio e conter um business.toml válido. O conteúdo é copiado para a raiz do estúdio com um id novo; a pasta original não é alterada.",
        "The folder must exist on the same machine that runs mads studio and contain a valid business.toml. Its content is copied to the studio root with a new id; the original folder is not changed.",
    ),
    (
        "A pasta sai da lista e vai para",
        "The folder leaves the list and moves to",
    ),
    ("A revisão aponta para", "The revision points to"),
    ("Abra", "Open"),
    ("Abrir a execução", "Open the run"),
    (
        "Aceita termos de busca, palavras-chave, campanhas, campanhas por dia e anúncios. Até 20 arquivos e 20 MB.",
        "Accepts search terms, keywords, campaigns, campaigns by day and ads. Up to 20 files and 20 MB.",
    ),
    ("Antes", "Before"),
    ("Anunciar só esta oferta", "Advertise only this offer"),
    (
        "Anunciar só esta oferta, não o negócio inteiro",
        "Advertise only this offer, not the whole business",
    ),
    ("Anúncios políticos na UE", "EU political ads"),
    ("Arquivado", "Archived"),
    ("Arquivar", "Archive"),
    (
        "Arquivar esconde o projeto da lista. Os arquivos continuam no disco e você pode reativar quando quiser.",
        "Archiving hides the project from the list. The files stay on disk and you can reactivate it whenever you want.",
    ),
    ("Arquivar projeto", "Archive project"),
    ("Arquivo", "File"),
    ("Arquivos CSV", "CSV files"),
    ("Arquivos ignorados", "Ignored files"),
    ("Ativo", "Active"),
    (
        "Brand terms (separe por vírgula)",
        "Brand terms (comma separated)",
    ),
    ("Brasil", "Brazil"),
    ("CPC máximo (opcional)", "Max CPC (optional)"),
    ("Caminho", "Path"),
    ("Caminho da pasta no servidor", "Folder path on the server"),
    ("Campanha", "Campaign"),
    ("Campanhas ignoradas", "Ignored campaigns"),
    ("Campo", "Field"),
    ("Categorias restritas", "Restricted categories"),
    ("Catálogo", "Catalog"),
    ("Com sinal", "Flagged"),
    ("Como ler", "How to read"),
    ("Concorrentes", "Competitors"),
    ("Configurações do projeto.", "Project settings."),
    ("Conteúdo sexual", "Sexual content"),
    (
        "Conversão configurada no site",
        "Conversion tracking set up on the site",
    ),
    ("Conversões", "Conversions"),
    (
        "Copie para o estúdio uma pasta que já contém um",
        "Copy to the studio a folder that already contains a",
    ),
    (
        "Corrija e reexporte. Abra Corrigir para editar a conta e criar uma revisão.",
        "Fix and re-export. Open Correct to edit the account and create a revision.",
    ),
    ("Criar manualmente", "Create manually"),
    ("Criar projeto", "Create project"),
    ("Custo/conv.", "Cost/conv."),
    ("Dados lidos", "Data read"),
    ("Decimal com vírgula", "Decimal comma"),
    (
        "Deixar o agente buscar na web",
        "Let the agent search the web",
    ),
    ("Depois", "After"),
    ("Descrição", "Description"),
    ("Diagnóstico", "Diagnosis"),
    ("Dias", "Days"),
    ("Dias na janela", "Days in window"),
    (
        "Ela foi gerada do zero, sem execução anterior.",
        "It was generated from scratch, without a previous run.",
    ),
    ("Escopo", "Scope"),
    (
        "Esta execução ainda não escreveu",
        "This run has not written",
    ),
    (
        "Esta execução não está concluída.",
        "This run is not complete.",
    ),
    (
        "Esta execução não tem workspace para corrigir.",
        "This run has no workspace to correct.",
    ),
    (
        "Esta execução não é uma otimização.",
        "This run is not an optimization.",
    ),
    (
        "Esta execução não é uma revisão.",
        "This run is not a revision.",
    ),
    (
        "Esta execução não é uma revisão. Não há execução anterior para comparar.",
        "This run is not a revision. There is no previous run to compare.",
    ),
    ("Estado", "State"),
    ("Estudar o site de novo", "Study the site again"),
    ("Estudar site", "Study site"),
    (
        "Estudar site e criar projeto",
        "Study site and create project",
    ),
    (
        "Estão nos relatórios, mas não nesta execução.",
        "They are in the reports but not in this run.",
    ),
    ("Evitar", "Avoid"),
    ("Excluir", "Delete"),
    (
        "Execução anterior não encontrada.",
        "Previous run not found.",
    ),
    ("Execução · alterações", "Run · changes"),
    ("Execução · correção", "Run · correction"),
    ("Execução · nova", "Run · new"),
    ("Execução · performance", "Run · performance"),
    ("Execução · relatório", "Run · report"),
    ("Exportação", "Export"),
    ("Formato", "Format"),
    ("Formatos obrigatórios", "Required formats"),
    ("Gerar", "Generate"),
    ("Gerar conta", "Generate account"),
    ("Id", "ID"),
    ("Identificação", "Identification"),
    ("Idioma", "Language"),
    ("Importar", "Import"),
    ("Importar pasta", "Import folder"),
    ("Importar projeto", "Import project"),
    ("Impressões / cliques", "Impressions / clicks"),
    ("Iniciar otimização", "Start optimization"),
    (
        "Isto gasta tokens do seu provedor.",
        "This spends your provider's tokens.",
    ),
    ("Isto gasta tokens.", "This spends tokens."),
    ("Itens do catálogo", "Catalog items"),
    ("Janela", "Window"),
    ("Janela e totais", "Window and totals"),
    ("Janelas diferentes.", "Different windows."),
    ("Jogos", "Gambling"),
    (
        "Leia os números antes de iniciar. O agente usa este digest para decidir orçamento e estrutura.",
        "Read the numbers before starting. The agent uses this digest to decide budget and structure.",
    ),
    ("Ler relatórios", "Read reports"),
    ("Linhas", "Rows"),
    (
        "Links simbólicos, a pasta .git e a pasta .mads não são copiados. O restante, incluindo out/, é copiado.",
        "Symbolic links, the .git folder and the .mads folder are not copied. Everything else, including out/, is copied.",
    ),
    ("Localização", "Location"),
    ("Missões em paralelo", "Missions in parallel"),
    ("Modelo", "Model"),
    ("Modelo de imagem (opcional)", "Image model (optional)"),
    ("Modelo do plano", "Plan model"),
    ("Modelo do plano (opcional)", "Plan model (optional)"),
    ("Moeda", "Currency"),
    ("Mover para a lixeira", "Move to trash"),
    ("Máximo de grupos", "Max ad groups"),
    ("Máximo de imagens", "Max images"),
    (
        "Nada é gerado agora. O projeto nasce com um rascunho válido e você pode revisar, estudar o site ou rodar uma execução depois.",
        "Nothing is generated now. The project starts with a valid draft and you can review it, study the site or start a run later.",
    ),
    (
        "Nenhum relatório lido nesta otimização.",
        "No reports read in this optimization.",
    ),
    ("Nenhuma alteração.", "No changes."),
    (
        "Nenhuma campanha nos relatórios.",
        "No campaigns in the reports.",
    ),
    (
        "Nenhuma verificação registrada. Pode ter sido pulada.",
        "No check recorded. It may have been skipped.",
    ),
    ("Nome do negócio", "Business name"),
    ("Novo projeto manual", "New manual project"),
    (
        "Não foi possível carregar as contas para comparar.",
        "Could not load the accounts to compare.",
    ),
    ("Não foi possível comparar.", "Could not compare."),
    ("O", "The"),
    (
        "O agent lê o site e pode pesquisar na web. Você acompanha e pode parar.",
        "The agent reads the site and can search the web. You follow along and can stop it.",
    ),
    (
        "O agente lê o site e escreve",
        "The agent reads the site and writes",
    ),
    (
        "O arquivo de chaves não carrega.",
        "The key file does not load.",
    ),
    (
        "O digest aparece antes de gastar tokens.",
        "The digest appears before spending tokens.",
    ),
    (
        "O navegador não lê o seu disco; o servidor lê o caminho.",
        "The browser does not read your disk; the server reads the path.",
    ),
    (
        "O projeto existe, mas o init não escreveu",
        "The project exists, but init did not write",
    ),
    (
        "O projeto vai para a lixeira, não é apagado.",
        "The project goes to the trash; it is not deleted.",
    ),
    (
        "O que está aqui vira o snapshot da próxima execução. Execuções antigas não mudam.",
        "What is here becomes the snapshot for the next run. Old runs do not change.",
    ),
    ("O que fica de fora", "What is left out"),
    (
        "O que o negócio vende, para quem e o que o diferencia.",
        "What the business sells, to whom, and what makes it different.",
    ),
    ("O rascunho não carrega.", "The draft does not load."),
    (
        "O rascunho vira um snapshot em",
        "The draft becomes a snapshot in",
    ),
    (
        "O rascunho, as execuções e o histórico ficam nesta pasta. Nada é enviado ao Google Ads.",
        "The draft, the runs and the history stay in this folder. Nothing is uploaded to Google Ads.",
    ),
    (
        "O servidor não conhece o preço. Você acompanha o teto e pode parar a execução.",
        "The server does not know the price. You watch the cap and can stop the run.",
    ),
    (
        "O workspace desta execução não carrega.",
        "This run's workspace does not load.",
    ),
    (
        "O workspace não guarda dados de performance.",
        "The workspace does not store performance data.",
    ),
    ("Objetivo", "Goal"),
    ("Onde ficam os arquivos", "Where the files live"),
    ("Orçamento diário", "Daily budget"),
    ("Orçamento ocioso", "Idle budget"),
    ("Orçamento ocioso/dia", "Idle budget/day"),
    (
        "Os eventos aparecem aqui. Se a conexão cair, recarregue a página.",
        "Events appear here. If the connection drops, reload the page.",
    ),
    (
        "Os relatórios cobrem períodos diferentes: os totais de um não fecham com as linhas do outro.",
        "The reports cover different periods: the totals of one do not match the rows of the other.",
    ),
    ("Política", "Politics"),
    (
        "Preencha o rascunho sem rodar o agente. O estúdio escreve o business.toml e valida com o mesmo parser do mads.",
        "Fill in the draft without running the agent. The studio writes business.toml and validates it with the same parser as mads.",
    ),
    ("Principal", "Main"),
    ("Projeto · rascunho", "Project · draft"),
    ("Projeto · rascunho ausente", "Project · draft missing"),
    ("Provedor de imagens", "Image provider"),
    ("Pular a verificação de URL", "Skip the URL check"),
    ("Reativar projeto", "Reactivate project"),
    ("Relatórios", "Reports"),
    ("Relatórios do Google Ads", "Google Ads reports"),
    ("Relatórios faltando", "Missing reports"),
    ("Retentativas", "Retries"),
    (
        "Revisar o plano antes de gastar nas campanhas",
        "Review the plan before spending on campaigns",
    ),
    ("Rode", "Run"),
    (
        "Salvar cria uma nova revisão.",
        "Saving creates a new revision.",
    ),
    ("Saúde", "Healthcare"),
    (
        "Se o init falhar de novo, o job mostra o erro. O projeto e as chaves continuam.",
        "If init fails again, the job shows the error. The project and the keys stay.",
    ),
    ("Sem conta.", "No account."),
    (
        "Sem estes relatórios, as decisões de palavra-chave ficam cegas.",
        "Without these reports, keyword decisions are blind.",
    ),
    (
        "Sem formato marcado, o agente escolhe.",
        "With no format selected, the agent chooses.",
    ),
    ("Sem provedor e sem tokens.", "No provider and no tokens."),
    ("Sem relatório", "No report"),
    ("Serviços financeiros", "Financial services"),
    ("Sinais", "Signals"),
    (
        "Suba os CSVs exportados da conta que já está no ar. Nada é enviado ao Google. A execução original não muda.",
        "Upload the CSVs exported from the live account. Nothing is sent to Google. The original run does not change.",
    ),
    ("Substituir arquivos existentes", "Replace existing files"),
    ("Tempo por missão", "Time per mission"),
    ("Teto de tokens", "Token cap"),
    ("Texto", "Text"),
    ("Turnos por missão", "Turns per mission"),
    (
        "Variáveis de ambiente têm precedência. O navegador nunca recebe o valor.",
        "Environment variables take precedence. The browser never receives the value.",
    ),
    ("Ver business.toml", "View business.toml"),
    ("Ver execução", "View run"),
    (
        "Você pode editar textos, orçamento, lances, URLs, palavras, negativas, grupos de assets e ativos de campanha. Para mudar a estrutura de campanhas ou grupos, gere outra execução.",
        "You can edit texts, budget, bids, URLs, keywords, negatives, asset groups and campaign assets. To change the campaign or group structure, create another run.",
    ),
    (
        "Você revisa os números e só então inicia as missões.",
        "You review the numbers and only then start the missions.",
    ),
    ("Voltar ao projeto", "Back to project"),
    ("alteração(ões) em relação à execução", "change(s) from run"),
    ("arquivo local", "local file"),
    ("arquivo(s)", "file(s)"),
    ("arquivos 1–5 + Editor", "files 1–5 + Editor"),
    ("campanha(s)", "campaign(s)"),
    (
        "cliques sem conversão visível.",
        "clicks without a visible conversion.",
    ),
    ("cole a chave", "paste the key"),
    (
        "com os relatórios do Google Ads para ler a conta que está no ar.",
        "with the Google Ads reports to read the live account.",
    ),
    (
        "converte e gasta todo o orçamento.",
        "converts and spends the whole budget.",
    ),
    (
        "custo que os termos de busca não mostram.",
        "cost that search terms do not show.",
    ),
    (
        "dados de menos. Só estrutura.",
        "too little data. Structure only.",
    ),
    ("descrições", "descriptions"),
    ("dia", "day"),
    ("e", "and"),
    ("falharam", "failed"),
    ("grupo(s)", "group(s)"),
    ("iniciado", "started"),
    ("lida(s)", "read"),
    ("limite", "limit"),
    (
        "mostra os mesmos estados no terminal. Logins de agentes CLI continuam sendo feitos fora do estúdio.",
        "shows the same states in the terminal. CLI agent logins are still done outside the studio.",
    ),
    ("no limite", "at the limit"),
    (
        "o id que o provedor documenta",
        "the id documented by the provider",
    ),
    (
        "o que faltou para decidir keyword.",
        "what was missing to decide a keyword.",
    ),
    (
        "orçamento que sobra todo dia.",
        "budget left over every day.",
    ),
    ("pacote de automação", "automation bundle"),
    ("padrão do provedor", "provider default"),
    ("palavras", "keywords"),
    (
        "para editar a conta e reexportar sem chamar a IA. Para mudar o rascunho e gerar outra execução, edite o Rascunho.",
        "to edit the account and re-export without calling the AI. To change the draft and generate another run, edit the Draft.",
    ),
    (
        "parcela perdida por orçamento / ranking.",
        "share lost to budget / rank.",
    ),
    ("perdido", "lost"),
    ("permissão", "permission"),
    ("presente", "present"),
    (
        "quebrada, sem anúncios ou impressões.",
        "broken, with no ads or impressions.",
    ),
    ("terminado", "finished"),
    ("termos ocultos", "hidden terms"),
    ("títulos", "headlines"),
    (
        "um modelo mais forte só para o plano",
        "a stronger model only for the plan",
    ),
    ("usado", "used"),
    ("vender vinho online", "sell wine online"),
    ("Álcool", "Alcohol"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Keys that the English UI depends on. These must never fall back to the Portuguese key.
    const CRITICAL: &[(&str, &str)] = &[
        ("Projetos", "Projects"),
        ("Configurações", "Settings"),
        ("Arquivado", "Archived"),
        ("Arquivar projeto", "Archive project"),
        ("Reativar projeto", "Reactivate project"),
        ("Importar projeto", "Import project"),
        ("Importar pasta", "Import folder"),
        ("Novo projeto manual", "New manual project"),
        ("Nome do negócio", "Business name"),
        ("Orçamento diário", "Daily budget"),
        ("Moeda", "Currency"),
        ("Idioma", "Language"),
        ("Descrição", "Description"),
        ("Execução · performance", "Run · performance"),
        ("Dias na janela", "Days in window"),
        ("Relatórios faltando", "Missing reports"),
        ("Custo/conv.", "Cost/conv."),
        ("Alterações", "Changes"),
        ("Escopo", "Scope"),
        ("Teto de tokens", "Token cap"),
        ("Retentativas", "Retries"),
        ("Salvar logo", "Save logo"),
        ("Salvar pesquisa", "Save research"),
        ("Salvar design", "Save design"),
        ("Grupos de assets", "Asset groups"),
        ("Títulos", "Headlines"),
        ("Conta", "Account"),
        ("Validação", "Validation"),
    ];

    #[test]
    fn critical_keys_have_an_english_translation() {
        for (key, expected) in CRITICAL {
            let english = t(EN, key);
            assert_ne!(english, *key, "missing English for {key:?}");
            assert_eq!(english, *expected, "wrong English for {key:?}");
        }
    }

    #[test]
    fn portuguese_returns_the_key_unchanged() {
        for (key, _) in CRITICAL {
            assert_eq!(t(PT_BR, key), *key);
        }
    }

    #[test]
    fn the_english_catalog_has_no_duplicates() {
        let mut seen = std::collections::BTreeSet::new();
        for (key, _) in EN_TEXT {
            assert!(seen.insert(*key), "duplicated catalog key: {key:?}");
        }
    }

    /// Spec section 13: no user-facing string lives in a template without an
    /// English translation. This fails when a template gains a
    /// `chrome.t("...")` whose key is not in `EN_TEXT`.
    #[test]
    fn every_template_key_has_an_english_translation() {
        let templates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
        let mut checked = 0usize;
        let mut files = 0usize;
        let entries =
            std::fs::read_dir(&templates).expect("the templates directory ships with the crate");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("html") {
                continue;
            }
            files += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            let bytes = text.as_bytes();
            let mut i = 0;
            while let Some(dot) = text[i..].find(".t(\"") {
                let start = i + dot + 4;
                let Some(len) = bytes[start..].iter().position(|&b| b == b'"') else {
                    break;
                };
                let key = &text[start..start + len];
                assert!(
                    EN_TEXT.iter().any(|(pt, _)| *pt == key),
                    "{}: {:?} has no English translation",
                    path.display(),
                    key
                );
                checked += 1;
                i = start + len;
            }
        }
        assert!(files > 0, "no templates found");
        assert!(checked > 300, "only {checked} keys found; the scan broke");
    }
}
