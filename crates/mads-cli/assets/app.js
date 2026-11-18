(() => {
  "use strict";

  const $ = (selector) => document.querySelector(selector);
  const lang = document.documentElement.lang === "en" ? "en" : "pt-BR";
  const en = {
    "Token inválido ou expirado. Copie o endereço impresso no terminal e tente de novo.":
      "Invalid or expired token. Copy the address printed in the terminal and try again.",
    "Não foi possível falar com o servidor. Ele ainda está rodando?":
      "Could not reach the server. Is it still running?",
    "O token tem pelo menos 32 caracteres.": "The token has at least 32 characters.",
    "Remover a chave salva de": "Remove the saved key for",
    "Nada mudou.": "Nothing changed.",
    "este projeto": "this project",
    "Parar esta execução? O trabalho já salvo permanece e pode ser retomado.":
      "Stop this run? Work already saved stays and can be resumed.",
    "concluída": "done",
    "falhou": "failed",
    "erro": "error",
    "erro(s)": "error(s)",
    "aviso(s)": "warning(s)",
    "sem resposta": "no response",
    "fim": "end",
    "Falhou": "Failed",
    "Concluído": "Done",
    "O job falhou. Recarregue a página para ver o erro completo.":
      "The job failed. Reload the page to see the full error.",
    "O job terminou. Recarregue a página para ver o resultado.":
      "The job finished. Reload the page to see the result.",
    "Na fila": "Queued",
    "Em execução": "Running",
    "Parando": "Stopping",
    "Cancelado": "Cancelled",
    "Desfazer esta revisão? Uma nova revisão volta à conta anterior e é reexportada.":
      "Undo this revision? A new revision restores the previous account and is exported again.",
    "in,": "in,",
    "out": "out",
    "plano pronto": "plan ready",
  };
  const tr = (text) => (lang === "en" ? en[text] || text : text);
  const showError = (text) => {
    const node = $("[data-bootstrap-error]");
    if (node) {
      node.textContent = text;
      node.hidden = false;
    }
  };

  async function login(token) {
    try {
      const response = await fetch("/api/session", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token }),
      });
      if (!response.ok) {
        // The token is wrong or the server restarted: never keep it in the URL.
        history.replaceState(null, "", location.pathname + location.search);
        showError(tr("Token inválido ou expirado. Copie o endereço impresso no terminal e tente de novo."));
        return;
      }
      history.replaceState(null, "", location.pathname + location.search);
      location.reload();
    } catch {
      showError(tr("Não foi possível falar com o servidor. Ele ainda está rodando?"));
    }
  }

  // Only a token-shaped fragment is a bootstrap token. A validation path such
  // as `campaigns[0].rsa.headlines[1]` is longer than the minimum token length
  // and must stay in the URL for the deep link to the fix field.
  const fragment = location.hash.slice(1);
  if (/^[A-Za-z0-9_-]{32,}$/.test(fragment)) {
    login(fragment);
  }

  const form = document.querySelector("[data-bootstrap]");
  if (form) {
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const data = new FormData(form);
      const token = String(data.get("token") || "").trim();
      if (token.length < 32) {
        showError(tr("O token tem pelo menos 32 caracteres."));
        return;
      }
      login(token);
    });
  }


  const csrf = () => {
    const meta = document.querySelector('meta[name="csrf-token"]');
    return meta ? meta.getAttribute("content") || "" : "";
  };

  async function postJson(url, body, extraHeaders) {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Mads-CSRF": csrf(),
        ...(extraHeaders || {}),
      },
      body: JSON.stringify(body || {}),
    });
    let payload = {};
    try {
      payload = await response.json();
    } catch {
      // A non JSON error still has a status.
    }
    if (!response.ok) {
      const message = payload && payload.error ? payload.error.message : `HTTP ${response.status}`;
      throw new Error(message);
    }
    return payload;
  }


  async function sendJson(method, url, body) {
    const response = await fetch(url, {
      method,
      headers: {
        "Content-Type": "application/json",
        "X-Mads-CSRF": csrf(),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    let payload = {};
    try {
      payload = await response.json();
    } catch {
      // Keep the HTTP status when the body is not JSON.
    }
    if (!response.ok) {
      const message = payload && payload.error ? payload.error.message : `HTTP ${response.status}`;
      throw new Error(message);
    }
    return payload;
  }

  document.querySelectorAll("[data-key-form]").forEach((form) => {
    const provider = form.dataset.provider;
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const input = form.querySelector('input[name="value"]');
      const error = form.querySelector("[data-key-error]");
      if (error) error.textContent = "";
      try {
        await sendJson("PUT", `/api/secrets/${provider}`, { value: input ? input.value : "" });
        location.reload();
      } catch (e) {
        if (error) error.textContent = e.message;
      }
    });
  });

  document.querySelectorAll("[data-key-delete]").forEach((button) => {
    button.addEventListener("click", async () => {
      const provider = button.dataset.keyDelete;
      if (!window.confirm(`${tr("Remover a chave salva de")} ${provider}?`)) return;
      try {
        await sendJson("DELETE", `/api/secrets/${provider}`);
        location.reload();
      } catch (e) {
        window.alert(e.message);
      }
    });
  });




  const revisionForm = document.querySelector("[data-revision-form]");
  if (revisionForm) {
    revisionForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const errorNode = revisionForm.querySelector("[data-form-error]");
      if (errorNode) errorNode.textContent = "";
      const edits = [];
      revisionForm.querySelectorAll("[data-path]").forEach((input) => {
        const value = input.value;
        if (value === input.dataset.original) return;
        edits.push({
          path: input.dataset.path,
          value: input.dataset.kind === "number" ? Number(value) : value,
        });
      });
      if (edits.length === 0) {
        if (errorNode) errorNode.textContent = tr("Nada mudou.");
        return;
      }
      try {
        const ifMatch = revisionForm.dataset.accountHash;
        const result = await postJson(
          revisionForm.dataset.action,
          { edits },
          ifMatch ? { "If-Match": ifMatch } : undefined,
        );
        location.href = `/jobs/${result.job_id}`;
      } catch (error) {
        if (errorNode) errorNode.textContent = error.message;
      }
    });
  }

  const optimizeForm = document.querySelector("[data-optimize-form]");
  if (optimizeForm) {
    optimizeForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const errorNode = optimizeForm.querySelector("[data-form-error]");
      if (errorNode) errorNode.textContent = "";
      const data = new FormData(optimizeForm);
      try {
        const response = await fetch(optimizeForm.dataset.action, {
          method: "POST",
          headers: { "X-Mads-CSRF": csrf() },
          body: data,
        });
        const payload = await response.json().catch(() => ({}));
        if (!response.ok) {
          throw new Error(payload.error ? payload.error.message : `HTTP ${response.status}`);
        }
        location.href = `/runs/${payload.run_id}/optimize/review`;
      } catch (error) {
        if (errorNode) errorNode.textContent = error.message;
      }
    });
  }



  document.querySelectorAll("[data-asset-form]").forEach((form) => {
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = form.querySelector("[data-form-error]");
      if (error) error.textContent = "";
      const text = form.querySelector('textarea[name="text"]');
      try {
        await sendJson("PUT", form.dataset.action, { text: text ? text.value : "" });
        location.reload();
      } catch (e) {
        if (error) error.textContent = e.message;
      }
    });
  });

  document.querySelectorAll("[data-logo-form]").forEach((form) => {
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = form.querySelector("[data-form-error]");
      if (error) error.textContent = "";
      try {
        const response = await fetch(form.dataset.action, {
          method: "POST",
          headers: { "X-Mads-CSRF": csrf() },
          body: new FormData(form),
        });
        const payload = await response.json().catch(() => ({}));
        if (!response.ok) {
          throw new Error(payload.error ? payload.error.message : `HTTP ${response.status}`);
        }
        location.reload();
      } catch (e) {
        if (error) error.textContent = e.message;
      }
    });
  });

  const draftForm = document.querySelector("[data-draft-form]");
  if (draftForm) {
    draftForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const errorNode = draftForm.querySelector("[data-form-error]");
      if (errorNode) errorNode.textContent = "";
      const form = new FormData(draftForm);
      const number = (name, fallback) => {
        const raw = form.get(name);
        if (raw === null || String(raw).trim() === "") return fallback;
        const value = Number(raw);
        return Number.isFinite(value) ? value : fallback;
      };
      const text = (name) => String(form.get(name) || "").trim();
      const body = {
        name: text("name"),
        url: text("url"),
        language: text("language"),
        location: text("location"),
        goal: text("goal"),
        description: text("description"),
        conversion_tracking: form.get("conversion_tracking") !== null,
        brand_terms: text("brand_terms"),
        competitors: text("competitors"),
        avoid: text("avoid"),
        restricted: form.getAll("restricted"),
        daily: number("daily", 0),
        currency: text("currency"),
        max_cpc: text("max_cpc") === "" ? null : number("max_cpc", 0),
        export_status: text("export_status") || "Paused",
        url_suffix: text("url_suffix"),
        eu_political_ads: form.get("eu_political_ads") !== null,
        decimal_comma: form.get("decimal_comma") !== null,
        formats: form.getAll("formats"),
        catalog_text: String(form.get("catalog_text") || ""),
        focus_name: text("focus_name"),
        focus_urls: String(form.get("focus_urls") || ""),
        focus_terms: String(form.get("focus_terms") || ""),
        app_store: text("app_store") || "google_play",
        app_id: text("app_id"),
        google_ads_customer_id: text("google_ads_customer_id"),
        google_ads_tracking_template: text("google_ads_tracking_template"),
        google_ads_devices: text("google_ads_devices"),
        google_ads_labels: text("google_ads_labels"),
        google_ads_app_id: text("google_ads_app_id"),
      };
      try {
        await sendJson("PUT", draftForm.dataset.action, body);
        location.reload();
      } catch (error) {
        if (errorNode) errorNode.textContent = error.message;
      }
    });
  }




  document.querySelectorAll("[data-project-archive]").forEach((button) => {
    button.addEventListener("click", async () => {
      const archived = button.dataset.archived === "true";
      try {
        await postJson(button.dataset.projectArchive, { archived });
        location.reload();
      } catch (error) {
        alert(error.message);
      }
    });
  });

  document.querySelectorAll("[data-project-delete]").forEach((button) => {
    button.addEventListener("click", async () => {
      const name = button.dataset.project || tr("este projeto");
      const typed = window.prompt(
        lang === "en"
          ? `Type the project name "${name}" to move it to the trash:`
          : `Digite o nome do projeto "${name}" para movê-lo à lixeira:`,
      );
      if (typed === null) return;
      try {
        await postJson(button.dataset.projectDelete, { confirm: typed.trim() });
        location.href = "/";
      } catch (error) {
        alert(error.message);
      }
    });
  });

  const renameForm = document.querySelector("[data-rename-form]");
  if (renameForm) {
    renameForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = renameForm.querySelector("[data-form-error]");
      if (error) error.textContent = "";
      const name = String(new FormData(renameForm).get("name") || "").trim();
      try {
        await postJson(renameForm.dataset.action, { name });
        location.reload();
      } catch (e) {
        if (error) error.textContent = e.message;
      }
    });
  }

  const importForm = document.querySelector("[data-project-import-form]");
  if (importForm) {
    importForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = importForm.querySelector("[data-form-error]");
      if (error) error.textContent = "";
      try {
        const result = await postJson(importForm.dataset.action, {
          path: new FormData(importForm).get("path"),
        });
        location.href = `/projects/${result.project_id}`;
      } catch (e) {
        if (error) error.textContent = e.message;
      }
    });
  }

  const manualForm = document.querySelector("[data-project-manual-form]");
  if (manualForm) {
    manualForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = manualForm.querySelector("[data-form-error]");
      if (error) error.textContent = "";
      const form = new FormData(manualForm);
      const number = (name, fallback) => {
        const raw = form.get(name);
        const value = raw === null ? NaN : Number(raw);
        return Number.isFinite(value) ? value : fallback;
      };
      try {
        const result = await postJson(manualForm.dataset.action, {
          name: String(form.get("name") || "").trim(),
          url: String(form.get("url") || "").trim(),
          daily: number("daily", 50),
          currency: String(form.get("currency") || "BRL").trim(),
          language: String(form.get("language") || "pt-BR").trim(),
          location: String(form.get("location") || "").trim(),
          goal: String(form.get("goal") || "").trim(),
          description: String(form.get("description") || "").trim(),
        });
        location.href = `/projects/${result.project_id}/draft`;
      } catch (e) {
        if (error) error.textContent = e.message;
      }
    });
  }


  const revert = document.querySelector("[data-revert-revision]");
  if (revert) {
    revert.addEventListener("click", async () => {
      if (!confirm(tr("Desfazer esta revisão? Uma nova revisão volta à conta anterior e é reexportada."))) return;
      try {
        const result = await postJson(revert.dataset.action, {});
        location.href = `/jobs/${result.job_id}`;
      } catch (error) {
        alert(error.message);
      }
    });
  }


  document.querySelectorAll('.anchor-nav a[href^="#"]').forEach((link) => {
    link.addEventListener("click", () => {
      const target = document.querySelector(link.getAttribute("href"));
      if (target && target.tagName === "DETAILS") target.open = true;
    });
  });
  if (location.hash) {
    try {
      const target = document.querySelector(location.hash);
      if (target && target.tagName === "DETAILS") target.open = true;
    } catch {
      // A fragment that is not a valid selector is only a bad anchor.
    }
    // A validation issue path lands on the matching field in Correct.
    const fieldPath = decodeURIComponent(location.hash.slice(1));
    const input = Array.from(document.querySelectorAll("[data-path]"))
      .find((node) => node.dataset.path === fieldPath);
    if (input) {
      input.scrollIntoView({ block: "center" });
      input.focus({ preventScroll: true });
      input.classList.add("field-target");
      setTimeout(() => input.classList.remove("field-target"), 6000);
    }
  }

  // Performance flags carry their practitioner rationale on hover.
  const flagHints = {
    thin: tr("poucos sinais: pouco tráfego na janela para julgar a campanha"),
    stalled: tr("parada: ligada sem anúncios, ou 7 dias sem uma impressão"),
    untracked: tr("sem conversão medida: 100+ cliques e nenhuma conversão enquanto outra campanha converte"),
    "no limite": tr("no limite: o orçamento já equivale ao que ela gastou por dia"),
    "termos ocultos": tr("termos ocultos: o Google omite alguns termos de busca por privacidade; os números são parciais"),
  };
  document.querySelectorAll(".chip[data-flag]").forEach((chip) => {
    const hint = flagHints[chip.dataset.flag];
    if (hint) chip.title = hint;
  });

  // Click a header to sort a data-sortable table; numbers compare numerically.
  document.querySelectorAll("table[data-sortable] thead th").forEach((th, index) => {
    th.classList.add("sortable");
    th.addEventListener("click", () => {
      const table = th.closest("table");
      const tbody = table.tBodies[0];
      const direction = th.dataset.dir === "asc" ? "desc" : "asc";
      table.querySelectorAll("thead th").forEach((other) => {
        other.removeAttribute("data-dir");
        other.removeAttribute("aria-sort");
      });
      th.dataset.dir = direction;
      th.setAttribute("aria-sort", direction === "asc" ? "ascending" : "descending");
      const number = (cell) => {
        const text = cell.textContent.trim().replace(/\s+/g, " ");
        if (!/\d/.test(text) || /^n\/a$/.test(text)) return null;
        const cleaned = text.replace(/[^\d.,-]/g, "").replace(/\.(?=\d{3}\b)/g, "").replace(",", ".");
        const value = parseFloat(cleaned);
        return Number.isFinite(value) ? value : null;
      };
      const rows = Array.from(tbody.rows);
      rows.sort((a, b) => {
        const va = number(a.cells[index]);
        const vb = number(b.cells[index]);
        let cmp;
        if (va !== null && vb !== null) cmp = va - vb;
        else if (va !== null) cmp = -1;
        else if (vb !== null) cmp = 1;
        else cmp = a.cells[index].textContent.trim().localeCompare(b.cells[index].textContent.trim());
        return direction === "asc" ? cmp : -cmp;
      });
      rows.forEach((row) => tbody.append(row));
    });
  });

  // Fills the model datalist from the provider's own models endpoint. A free
  // text id always works; the list is a convenience.
  document.querySelectorAll("[data-provider-select]").forEach((select) => {
    const form = select.closest("form");
    const datalist = form && form.querySelector("#model-options");
    const hint = form && form.querySelector("[data-model-hint]");
    const baseUrl = form && form.querySelector("[data-base-url]");
    if (!datalist) return;
    // Fills the endpoint field with the provider's documented default. A value
    // the user typed (or edited) is never overwritten.
    const prefillEndpoint = () => {
      if (!baseUrl) return;
      const option = select.selectedOptions[0];
      const fallback = option ? option.dataset.defaultEndpoint : "";
      if (!fallback) return;
      const previous = baseUrl.dataset.autoFilled || "";
      if (baseUrl.value.trim() === "" || baseUrl.value.trim() === previous) {
        baseUrl.value = fallback;
        baseUrl.dataset.autoFilled = fallback;
      }
    };
    prefillEndpoint();

    const load = async () => {
      const provider = select.value;
      const cliProvider = provider.endsWith("-cli");
      prefillEndpoint();
      if (!provider || cliProvider) {
        datalist.replaceChildren();
        if (hint) {
          hint.textContent = cliProvider ? tr("claude-cli, codex-cli e gemini-cli escolhem o modelo deles") : "";
        }
        return;
      }
      if (hint) hint.textContent = tr("buscando modelos") + "…";
      const endpoint = new URL(`/api/providers/${encodeURIComponent(provider)}/models`, location.origin);
      if (baseUrl && baseUrl.value.trim()) {
        endpoint.searchParams.set("base_url", baseUrl.value.trim());
      }
      try {
        const response = await fetch(endpoint);
        const payload = await response.json().catch(() => ({}));
        if (!response.ok) {
          throw new Error(payload.error ? payload.error.message : `HTTP ${response.status}`);
        }
        const options = (payload.models || []).map((id) => {
          const option = document.createElement("option");
          option.value = id;
          return option;
        });
        datalist.replaceChildren(...options);
        if (hint) hint.textContent = options.length ? "" : tr("Nada mudou.");
      } catch (error) {
        datalist.replaceChildren();
        if (hint) {
          const rejected = /HTTP 40[13]/.test(error.message);
          hint.textContent = rejected
            ? tr("a chave foi rejeitada pelo provedor; confira em Conexões")
            : error.message;
          hint.title = error.message;
        }
      }
    };
    select.addEventListener("change", load);
    if (baseUrl) baseUrl.addEventListener("change", load);
    if (select.value) load();
  });

  const language = document.querySelector("[data-language]");
  if (language) {
    language.addEventListener("change", () => {
      const next = location.pathname + location.search;
      location.href = `/settings/language?lang=${encodeURIComponent(language.value)}&next=${encodeURIComponent(next)}`;
    });
  }

  const initForm = document.querySelector("[data-init-form]");
  if (initForm) {
    initForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const errorNode = initForm.querySelector("[data-form-error]");
      if (errorNode) errorNode.textContent = "";
      const form = new FormData(initForm);
      const number = (name, fallback) => {
        const raw = form.get(name);
        if (raw === null || String(raw).trim() === "") return fallback;
        const value = Number(raw);
        return Number.isFinite(value) ? value : fallback;
      };
      const text = (name) => String(form.get(name) || "").trim();
      const body = {
        start_url: text("start_url"),
        daily: number("daily", 50),
        currency: text("currency"),
        provider: text("provider"),
        model: text("model") || null,
        base_url: text("base_url") || null,
        catalog_limit: number("catalog_limit", 50),
        web_search: form.get("web_search") !== null,
        focus: form.get("focus") !== null,
        force: form.get("force") !== null,
        max_turns: number("max_turns", 40),
        timeout: text("timeout") || "15m",
        max_tokens: number("max_tokens", 4000000),
        retries: number("retries", 1),
      };
      try {
        const result = await postJson(initForm.dataset.action, body);
        location.href = `/jobs/${result.job_id}`;
      } catch (error) {
        if (errorNode) errorNode.textContent = error.message;
      }
    });
  }

  const projectForm = document.querySelector("[data-project-form]");
  if (projectForm) {
    projectForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const errorNode = projectForm.querySelector("[data-form-error]");
      if (errorNode) errorNode.textContent = "";
      const form = new FormData(projectForm);
      const number = (name, fallback) => {
        const raw = form.get(name);
        if (raw === null || String(raw).trim() === "") return fallback;
        const value = Number(raw);
        return Number.isFinite(value) ? value : fallback;
      };
      const body = {
        name: String(form.get("name") || "").trim(),
        start_url: String(form.get("start_url") || "").trim(),
        daily: number("daily", 50),
        currency: String(form.get("currency") || "BRL").trim(),
        provider: String(form.get("provider") || "").trim(),
        model: String(form.get("model") || "").trim() || null,
        base_url: String(form.get("base_url") || "").trim() || null,
        catalog_limit: number("catalog_limit", 50),
        web_search: form.get("web_search") !== null,
        focus: form.get("focus") !== null,
        max_turns: number("max_turns", 40),
        timeout: String(form.get("timeout") || "15m"),
        max_tokens: number("max_tokens", 4000000),
        retries: number("retries", 1),
      };
      try {
        const result = await postJson(projectForm.dataset.action, body);
        location.href = `/jobs/${result.job_id}`;
      } catch (error) {
        if (errorNode) errorNode.textContent = error.message;
      }
    });
  }

  const runForm = document.querySelector("[data-run-form]");
  if (runForm) {
    runForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const errorNode = runForm.querySelector("[data-form-error]");
      if (errorNode) errorNode.textContent = "";
      const form = new FormData(runForm);
      const number = (name, fallback) => {
        const raw = form.get(name);
        if (raw === null || String(raw).trim() === "") return fallback;
        const value = Number(raw);
        return Number.isFinite(value) ? value : fallback;
      };
      const body = {
        provider: String(form.get("provider") || "").trim(),
        model: String(form.get("model") || "").trim() || null,
        plan_model: String(form.get("plan_model") || "").trim() || null,
        base_url: String(form.get("base_url") || "").trim() || null,
        image_provider: String(form.get("image_provider") || "auto"),
        image_model: String(form.get("image_model") || "").trim() || null,
        layout: String(form.get("layout") || "bulk"),
        parallel: number("parallel", 4),
        max_turns: number("max_turns", 40),
        timeout: String(form.get("timeout") || "15m"),
        max_tokens: number("max_tokens", 4000000),
        retries: number("retries", 1),
        max_ad_groups: number("max_ad_groups", 50),
        max_images: number("max_images", 40),
        skip_url_check: form.get("skip_url_check") !== null,
        plan_review: form.get("plan_review") !== null,
      };
      try {
        const job = await postJson(runForm.dataset.action, body);
        location.href = `/jobs/${job.id}`;
      } catch (error) {
        if (errorNode) errorNode.textContent = error.message;
      }
    });
  }

  document.querySelectorAll("[data-cancel-job]").forEach((button) => {
    button.addEventListener("click", async () => {
      if (!window.confirm(tr("Parar esta execução? O trabalho já salvo permanece e pode ser retomado."))) return;
      button.disabled = true;
      try {
        await postJson(button.dataset.cancelJob, {});
      } catch (error) {
        button.disabled = false;
        window.alert(error.message);
      }
    });
  });

  document.querySelectorAll("[data-run-action]").forEach((button) => {
    button.addEventListener("click", async () => {
      const run = button.dataset.run;
      const action = button.dataset.runAction;
      let url = `/api/runs/${run}/export`;
      let body = {};
      if (action === "resume") {
        url = `/api/runs/${run}/resume`;
        body = { provider: button.dataset.provider || "", model: button.dataset.model || null };
      } else if (action === "continue") {
        url = `/api/runs/${run}/continue`;
        body = { provider: button.dataset.provider || "", model: button.dataset.model || null };
      }
      button.disabled = true;
      try {
        const job = await postJson(url, body);
        location.href = `/jobs/${job.id}`;
      } catch (error) {
        button.disabled = false;
        window.alert(error.message);
      }
    });
  });

  const feed = document.querySelector("[data-stream]");
  if (!feed) return;

  let last = Number(feed.dataset.last || "0");
  let follow = true;
  const stream = new EventSource(feed.dataset.stream);
  const finishJob = (failed, error) => {
    stream.close();
    clearInterval(poll);
    const status = document.querySelector(".page-head .status");
    if (status) {
      status.textContent = failed ? tr("Falhou") : tr("Concluído");
      status.className = `status status-${failed ? "error" : "ok"}`;
    }
    const hint = document.querySelector("[data-job-hint]");
    if (hint) {
      hint.textContent = error
        ? error
        : failed
          ? tr("O job falhou. Recarregue a página para ver o erro completo.")
          : tr("O job terminou. Recarregue a página para ver o resultado.");
    }
  };

  const jobId = feed.dataset.job;
  const poll = jobId
    ? setInterval(async () => {
        try {
          const response = await fetch(`/api/jobs/${jobId}`);
          if (!response.ok) return;
          const job = await response.json();
          if (job.state === "Failed" || job.state === "Cancelled") {
            finishJob(true, job.error_text || "");
          } else if (job.state === "Done") {
            finishJob(false, "");
          }
        } catch {
          // The next tick can try again.
        }
      }, 3000)
    : null;

  const append = (envelope) => {
    const event = envelope.event || {};
    const type = event.type || "event";
    const row = document.createElement("div");
    row.className = "event event-muted";
    row.dataset.seq = String(envelope.seq);

    const time = document.createElement("span");
    time.className = "event-time mono";
    time.textContent = (envelope.ts || "").slice(11, 19);

    const mission = document.createElement("span");
    mission.className = "event-mission mono";
    mission.textContent = event.mission || "run";

    const kind = document.createElement("span");
    kind.className = "event-kind";
    kind.textContent = type;

    const text = document.createElement("span");
    text.className = "event-text";
    text.textContent = describe(event);

    row.append(time, mission, kind, text);
    feed.append(row);
    // A long run must not grow the DOM without bound.
    while (feed.childElementCount > 500) {
      feed.firstElementChild.remove();
    }
    if (follow) {
      window.scrollTo({ top: document.body.scrollHeight });
    }
  };

  const describe = (event) => {
    switch (event.type) {
      case "run_started":
        return lang === "en" ? `start (${event.provider || "?"})` : `início (${event.provider || "?"})`;
      case "mission_started":
        return lang === "en" ? `attempt ${event.attempt}` : `tentativa ${event.attempt}`;
      case "mission_finished":
        return event.ok ? tr("concluída") : `${tr("falhou")}: ${event.reason || "?"}`;
      case "tool_called":
        return `> ${event.tool} ${event.summary || ""}`;
      case "tool_finished":
        return `< ${event.ok ? "ok" : tr("erro")} ${event.tool} ${event.summary || ""}`;
      case "usage":
        return `${event.input_tokens} ${tr("in,")} ${event.output_tokens} ${tr("out")}`;
      case "step":
        return `${event.name}: ${event.detail || ""}`;
      case "validation":
        return `${(event.errors || []).length} ${tr("erro(s)")}, ${(event.warnings || []).length} ${tr("aviso(s)")}`;
      case "url_checked":
        return `${event.url} ${event.status || tr("sem resposta")}`;
      case "artifact_written":
        return event.path;
      case "run_paused":
        return tr("plano pronto");
      case "run_finished":
        return `${tr("fim")} (exit ${event.exit_code})`;
      case "agent_text":
        return String(event.text || "").slice(0, 240);
      default:
        return "";
    }
  };

  stream.onmessage = (message) => {
    try {
      const envelope = JSON.parse(message.data);
      if (!envelope || Number(envelope.seq) <= last) return;
      last = Number(envelope.seq);
      append(envelope);
      if (envelope.event && envelope.event.type === "run_finished") {
        finishJob(Number(envelope.event.exit_code || 0) !== 0, "");
      }
    } catch {
      // Ignore a malformed frame. The next one can still be useful.
    }
  };

  stream.onerror = () => {
    // EventSource reconnects on its own. Do not spam the page.
  };

  window.addEventListener("scroll", () => {
    const nearBottom = window.innerHeight + window.scrollY >= document.body.scrollHeight - 40;
    follow = nearBottom;
  });
})();
