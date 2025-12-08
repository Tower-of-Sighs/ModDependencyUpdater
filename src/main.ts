import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { openPath as openExternal } from "@tauri-apps/plugin-opener";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
 

// Elements
const gradlePathInput = document.getElementById("gradle-path") as HTMLInputElement;
const browseBtn = document.getElementById("browse-btn") as HTMLButtonElement;
const sourceSelect = document.getElementById("source-select") as HTMLSelectElement;
const projectIdInput = document.getElementById("project-id") as HTMLInputElement;
const projectLabel = document.getElementById("project-label") as HTMLLabelElement;
const mcVersionSelect = document.getElementById("mc-version") as HTMLSelectElement;
const loaderSelect = document.getElementById("loader-select") as HTMLSelectElement;
const cfApiKeyInput = document.getElementById("cf-api-key") as HTMLInputElement;
const updateBtn = document.getElementById("update-btn") as HTMLButtonElement;
const logOutput = document.getElementById("log-output") as HTMLDivElement;
const apiKeyGroup = document.getElementById("api-key-group") as HTMLDivElement;
const modeSelect = document.getElementById("mode-select") as HTMLSelectElement;
const batchGroup = document.getElementById("batch-group") as HTMLDivElement;
const batchItems = document.getElementById("batch-items") as HTMLTextAreaElement;
const fetchOptionsBtn = document.getElementById("fetch-options") as HTMLButtonElement;
const langSelect = document.getElementById("lang-select") as HTMLSelectElement;
const projectGroup = document.getElementById("project-group") as HTMLDivElement;
const mcVersionSelectGroup = document.getElementById("mc-version-select-group") as HTMLDivElement;
const loaderSelectGroup = document.getElementById("loader-select-group") as HTMLDivElement;
const mcVersionInputGroup = document.getElementById("mc-version-input-group") as HTMLDivElement;
const loaderInputGroup = document.getElementById("loader-input-group") as HTMLDivElement;
const mcVersionInput = document.getElementById("mc-version-input") as HTMLInputElement;
const loaderInput = document.getElementById("loader-input") as HTMLInputElement;
const clearLogBtn = document.getElementById("clear-log-btn") as HTMLButtonElement;
const saveLogBtn = document.getElementById("save-log-btn") as HTMLButtonElement;
const versionModal = document.getElementById("version-modal") as HTMLDivElement;
const versionList = document.getElementById("version-list") as HTMLDivElement;
const modStrip = document.getElementById("mod-strip") as HTMLDivElement;
const versionApplyAllBtn = document.getElementById("version-apply-all") as HTMLButtonElement;
const versionApplyBtn = document.getElementById("version-apply") as HTMLButtonElement;
const versionCancelBtn = document.getElementById("version-cancel") as HTMLButtonElement;
const openLogBtn = document.getElementById("open-log-btn") as HTMLButtonElement;

type ProjectOptions = {
  versions: string[];
  loaders: string[];
  slug?: string;
  id?: number;
  version_to_loaders?: Record<string, string[]>;
  loader_to_versions?: Record<string, string[]>;
};

let currentDict: Record<string, string> = {};
let versionToLoaders: Record<string, string[]> = {};
let loaderToVersions: Record<string, string[]> = {};
let allVersions: string[] = [];
let allLoaders: string[] = [];

function t(key: string, fallback: string, params?: Record<string, string | number>) {
  const raw = currentDict[key] || fallback;
  if (!params) return raw;
  return Object.keys(params).reduce((acc, k) => acc.replace(new RegExp(`\\{${k}\\}`, "g"), String(params[k])), raw);
}

async function applyI18n(lang: string) {
  const langKey = lang === "zh-CN" ? "zh-CN" : "en";
  const res = await fetch(`/locales/${langKey}.json`);
  const dict = await res.json();
  currentDict = dict;
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach(el => {
    const key = el.getAttribute("data-i18n")!;
    if (dict[key]) el.textContent = dict[key];
  });
  document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("[data-i18n-placeholder]").forEach(el => {
    const key = el.getAttribute("data-i18n-placeholder")!;
    if (dict[key]) el.placeholder = dict[key];
  });
}

 

// Helper to log
function log(msg: string, isError = false, append = true) {
  logOutput.textContent = append && logOutput.textContent ? `${logOutput.textContent}\n${msg}` : msg;
  logOutput.style.color = isError ? "#ff6b6b" : "inherit";
  if (isError && window.matchMedia("(prefers-color-scheme: light)").matches) {
    logOutput.style.color = "#d32f2f";
  }
}

// Load saved state
function loadState() {
  const saved = localStorage.getItem("mod-dep-updater-state");
  if (saved) {
    const state = JSON.parse(saved);
    if (state.gradlePath) gradlePathInput.value = state.gradlePath;
    if (state.source) sourceSelect.value = state.source;
    if (state.projectId) projectIdInput.value = state.projectId;
    if (state.mcVersion) mcVersionSelect.value = state.mcVersion;
    if (state.loader) loaderSelect.value = state.loader;
    if (state.cfApiKey) cfApiKeyInput.value = state.cfApiKey;
    if (state.lang) langSelect.value = state.lang;
    if (state.mode) modeSelect.value = state.mode;
    
    updateUIState();
  }
}

// Save state
function saveState() {
  const state = {
    gradlePath: gradlePathInput.value,
    source: sourceSelect.value,
    projectId: projectIdInput.value,
    mcVersion: mcVersionSelect.value,
    loader: loaderSelect.value,
    cfApiKey: cfApiKeyInput.value,
    lang: langSelect.value,
    mode: modeSelect.value,
  };
  localStorage.setItem("mod-dep-updater-state", JSON.stringify(state));
}

// Update UI based on source selection
function updateUIState() {
  const source = sourceSelect.value;
  if (source === "curseforge") {
    projectLabel.textContent = currentDict["label_project_id_number"] || "Project ID (Number)";
    apiKeyGroup.style.display = "flex";
  } else {
    projectLabel.textContent = currentDict["label_project_id_slug"] || "Project Slug / ID";
    apiKeyGroup.style.display = "none";
  }
  const isBatch = modeSelect.value === "batch";
  batchGroup.style.display = isBatch ? "block" : "none";
  projectGroup.style.display = isBatch ? "none" : "block";
  if (fetchOptionsBtn) fetchOptionsBtn.style.display = isBatch ? "none" : "inline-block";
  mcVersionSelectGroup.style.display = isBatch ? "none" : "block";
  loaderSelectGroup.style.display = isBatch ? "none" : "block";
  mcVersionInputGroup.style.display = isBatch ? "block" : "none";
  loaderInputGroup.style.display = isBatch ? "block" : "none";
}

function setOptions(select: HTMLSelectElement, values: string[]) {
  select.innerHTML = "";
  values.forEach(v => {
    const opt = document.createElement("option");
    opt.value = v;
    opt.textContent = v;
    select.appendChild(opt);
  });
}

async function fetchProjectOptions() {
  if (!isTauriReady()) {
    log(t("log_tauri_not_ready_options", "Tauri not ready: cannot fetch project options"), true);
    return;
  }
  if (modeSelect.value === "batch") {
    return;
  }
  const source = sourceSelect.value;
  const projectId = projectIdInput.value.trim();
  const cfApiKey = cfApiKeyInput.value.trim();
  if (!projectId) {
    log(t("log_missing_project_id", "Please enter a Project ID."), true);
    return;
  }
  try {
    const result = await invoke<ProjectOptions>("get_project_options", {
      source,
      projectId,
      cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
    });
    const versions = result.versions || [];
    const loaders = result.loaders || [];
    versionToLoaders = result.version_to_loaders || {};
    loaderToVersions = result.loader_to_versions || {};
    allVersions = versions;
    allLoaders = loaders;
    setOptions(mcVersionSelect, versions);
    setOptions(loaderSelect, loaders);
    saveState();
    log(t("log_parsed_options", "Parsed options: versions {v}, loaders {l}", { v: versions.length, l: loaders.length }));
  } catch (err) {
    log(t("log_parse_failed", "Parse failed: {err}", { err: String(err) }), true);
  }
}

function clearOptions() {
  setOptions(mcVersionSelect, []);
  setOptions(loaderSelect, []);
  versionToLoaders = {};
  loaderToVersions = {};
}

function bindEvents() {
  if (sourceSelect) sourceSelect.addEventListener("change", updateUIState);
  if (sourceSelect) sourceSelect.addEventListener("change", clearOptions);
  if (modeSelect) modeSelect.addEventListener("change", updateUIState);
  if (langSelect) langSelect.addEventListener("change", () => {
    applyI18n(langSelect.value).then(() => {
      updateUIState();
      saveState();
    });
  });
  if (projectIdInput) projectIdInput.addEventListener("input", clearOptions);
  if (fetchOptionsBtn) fetchOptionsBtn.addEventListener("click", fetchProjectOptions);
  if (mcVersionSelect) mcVersionSelect.addEventListener("change", () => {
    const ver = mcVersionSelect.value;
    const loaders = versionToLoaders[ver] || [];
    const targetLoaders = loaders.length ? loaders : allLoaders;
    setOptions(loaderSelect, targetLoaders);
    if (!targetLoaders.includes(loaderSelect.value)) {
      loaderSelect.value = targetLoaders[0] || "";
    }
    const lv = loaderSelect.value;
    const allowedVersions = loaderToVersions[lv] || [];
    const targetVersions = allowedVersions.length ? allowedVersions : allVersions;
    setOptions(mcVersionSelect, targetVersions);
    if (!targetVersions.includes(ver)) {
      mcVersionSelect.value = targetVersions[0] || "";
    } else {
      mcVersionSelect.value = ver;
    }
    saveState();
    
    log(t("log_version_change", "Version: {ver} → loaders {lc}, versions {vc}", { ver, lc: targetLoaders.length, vc: targetVersions.length }));
  });
  if (loaderSelect) loaderSelect.addEventListener("change", () => {
    const loader = loaderSelect.value;
    const versions = loaderToVersions[loader] || [];
    const targetVersions = versions.length ? versions : allVersions;
    const currentVer = mcVersionSelect.value;
    setOptions(mcVersionSelect, targetVersions);
    if (!targetVersions.includes(currentVer)) {
      mcVersionSelect.value = targetVersions[0] || "";
    } else {
      mcVersionSelect.value = currentVer;
    }
    const verLoaders = versionToLoaders[mcVersionSelect.value] || [];
    const targetLoaders = verLoaders.length ? verLoaders : allLoaders;
    setOptions(loaderSelect, targetLoaders);
    if (!targetLoaders.includes(loader)) {
      loaderSelect.value = targetLoaders[0] || "";
    } else {
      loaderSelect.value = loader;
    }
    saveState();
    log(t("log_loader_change", "Loader changed: {loader}", { loader }));
  });
  if (browseBtn) browseBtn.addEventListener("click", async () => {
    if (!isTauriReady()) {
      log(t("log_tauri_not_ready_open", "Tauri not ready: cannot open file dialog"), true);
      return;
    }
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Gradle Files',
          extensions: ['gradle']
        }, {
          name: 'All Files',
          extensions: ['*']
        }]
      });
      if (selected) {
        gradlePathInput.value = selected as string;
        saveState();
        clearOptions();
      }
    } catch (err) {
      log(t("log_select_file_error", "Error selecting file: {err}", { err: String(err) }), true);
    }
  });
  if (clearLogBtn) clearLogBtn.addEventListener("click", () => { logOutput.textContent = ""; });
  if (saveLogBtn) saveLogBtn.addEventListener("click", async () => {
    if (!isTauriReady()) { log(t("log_tauri_not_ready_save_log", "Tauri not ready: cannot save log"), true); return; }
    try {
      const path = await invoke<string>("save_log", { content: logOutput.textContent || "" });
      log(t("log_saved_log", "Saved log: {path}", { path }));
    } catch (err) {
      log(t("log_save_failed", "Save failed: {err}", { err: String(err) }), true);
    }
  });
  if (openLogBtn) openLogBtn.addEventListener("click", async () => {
    if (!isTauriReady()) { log(t("log_tauri_not_ready_invoke", "Tauri not ready: cannot invoke backend commands"), true); return; }
    try {
      const dir = await invoke<string>("get_log_dir");
      await openExternal(dir);
      log(t("log_open_logs_dir", "Opened logs folder: {dir}", { dir }));
    } catch (err) {
      log(t("log_open_logs_failed", "Open logs folder failed: {err}", { err: String(err) }), true);
    }
  });
  if (updateBtn) updateBtn.addEventListener("click", async () => {
    if (!isTauriReady()) {
      log(t("log_tauri_not_ready_invoke", "Tauri not ready: cannot invoke backend commands"), true);
      return;
    }
    log(t("log_running_update", "Running update..."), false);
    const gradlePath = gradlePathInput.value.trim();
    const source = sourceSelect.value;
    const projectId = projectIdInput.value.trim();
    const isBatch = modeSelect.value === "batch";
    const mcVersion = isBatch ? mcVersionInput.value.trim() : mcVersionSelect.value;
    const loader = isBatch ? loaderInput.value.trim() : loaderSelect.value;
    const cfApiKey = cfApiKeyInput.value.trim();
    if (!gradlePath) {
      log(t("log_select_gradle", "Please select a build.gradle file."), true);
      return;
    }
    if (!isBatch && !projectId) {
      log(t("log_enter_project_id", "Please enter a Project ID."), true);
      return;
    }
    if (isBatch && (!mcVersion || !loader)) {
      log(t("log_batch_need_fields", "In batch mode, please fill version and loader"), true);
      return;
    }
    if (source === "curseforge" && !cfApiKey) {
      log(t("log_no_api_key_warning", "Warning: No API Key provided. If not set in environment variables, this will fail."), false);
    }
    saveState();
    try {
      if (!isBatch) {
        const res = await invoke<{ choices: { id: string; label: string; kind: string }[] }>("list_versions", {
          source,
          projectId,
          mcVersion,
          loader,
          cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
        });
        const choices = res.choices || [];
        if (choices.length === 0) { log(t("log_no_versions_found", "No matching versions found"), true); return; }
        versionList.innerHTML = "";
        choices.forEach((c, idx) => {
          const item = document.createElement("div");
          item.className = "version-item";
          const id = `ver-${idx}`;
          const radio = document.createElement("input");
          radio.type = "radio";
          radio.name = "version-choice";
          radio.value = c.id;
          radio.id = id;
          if (idx === 0) radio.checked = true;
          const label = document.createElement("label");
          label.htmlFor = id;
          label.textContent = c.label;
          item.appendChild(radio);
          item.appendChild(label);
          versionList.appendChild(item);
        });
        versionModal.style.display = "flex";
        const applyHandler = async () => {
          const sel = document.querySelector<HTMLInputElement>('input[name="version-choice"]:checked');
          if (!sel) { log(t("log_please_select_version", "Please select a version"), true); return; }
          versionApplyBtn.disabled = true;
          try {
            const result = await invoke<string>("apply_selected_version", {
              gradlePath,
              source,
              projectId,
              loader,
              selectedId: sel.value,
              cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
            });
            log(result);
            versionModal.style.display = "none";
          } catch (err) {
            log(t("log_apply_failed", "Apply failed: {err}", { err: String(err) }), true);
          } finally {
            versionApplyBtn.disabled = false;
            versionApplyBtn.removeEventListener("click", applyHandler);
            versionCancelBtn.removeEventListener("click", cancelHandler);
          }
        };
        const cancelHandler = () => {
          versionModal.style.display = "none";
          versionApplyBtn.removeEventListener("click", applyHandler);
          versionCancelBtn.removeEventListener("click", cancelHandler);
        };
        versionApplyBtn.addEventListener("click", applyHandler);
        versionCancelBtn.addEventListener("click", cancelHandler);
      } else {
        const items = batchItems.value
          .split(/\r?\n/)
          .map(s => s.trim())
          .filter(Boolean);
        if (items.length === 0) {
          log(t("log_batch_enter_projects", "Enter project list in batch mode"), true);
          return;
        }
        try {
          const t0 = performance.now();
          const briefs = await invoke<{ mods: { key: string; name: string; icon: string; icon_data: string }[] }>("get_batch_mod_briefs", {
            source,
            items,
            cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
          });
          modStrip.innerHTML = "";
          versionList.innerHTML = "";
          let currentKey = "";
          const versionsCache: Record<string, { id: string; label: string; kind: string }[]> = {};
          const selectedVersionByMod: Record<string, string> = {};
          const mods = briefs.mods || [];
          const toUrl = (dataUrl: string, p: string) => {
            if (dataUrl) return dataUrl;
            if (!p) return "/src/assets/app-icon.ico";
            if (/^https?:/i.test(p)) return p;
            const normalized = p.replace(/\\/g, "/");
            return convertFileSrc(normalized);
          };
          const renderChoices = (key: string, choices: { id: string; label: string; kind: string }[]) => {
            versionList.innerHTML = "";
            choices.forEach((c, idx) => {
              const id = `ver-${key}-${idx}`;
              const item = document.createElement("div");
              item.className = "version-item";
              const radio = document.createElement("input");
              radio.type = "radio";
              radio.name = "version-choice";
              radio.value = c.id;
              radio.id = id;
              radio.checked = selectedVersionByMod[key] ? (selectedVersionByMod[key] === c.id) : (idx === 0);
              radio.addEventListener("change", () => { selectedVersionByMod[key] = c.id; });
              const label = document.createElement("label");
              label.htmlFor = id;
              label.textContent = c.label;
              item.appendChild(radio);
              item.appendChild(label);
              versionList.appendChild(item);
            });
          };
          mods.forEach((m, idx) => {
            const card = document.createElement("div");
            card.className = "mod-card";
            const img = document.createElement("img");
            img.loading = "lazy";
            img.src = toUrl(m.icon_data, m.icon);
            const span = document.createElement("div");
            span.className = "label";
            span.textContent = m.name || m.key;
            card.appendChild(img);
            card.appendChild(span);
            card.addEventListener("click", async () => {
              document.querySelectorAll(".mod-card").forEach(el => el.classList.remove("active"));
              card.classList.add("active");
              currentKey = m.key;
              let choices = versionsCache[m.key];
              if (!choices) {
                versionList.textContent = t("log_loading", "Loading...");
                const t1 = performance.now();
                try {
                  const res = await invoke<{ choices: { id: string; label: string; kind: string }[] }>("list_versions", {
                    source,
                    projectId: m.key,
                    mcVersion,
                    loader,
                    cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
                  });
                  choices = res.choices || [];
                  versionsCache[m.key] = choices;
                  log(t("log_versions_loaded", "Versions for {key} loaded in {ms}ms (count {n})", { key: m.key, ms: Math.round(performance.now() - t1), n: choices.length }));
                } catch (err) {
                  log(t("log_parse_failed", "Parse failed: {err}", { err: String(err) }), true);
                  return;
                }
              }
              renderChoices(m.key, choices);
            });
            modStrip.appendChild(card);
            if (idx === 0) { card.classList.add("active"); currentKey = m.key; }
          });
          versionModal.style.display = "flex";
          log(t("log_batch_modal_show", "Showing batch modal: {n} mods", { n: mods.length }));
          const firstCard = modStrip.querySelector(".mod-card") as HTMLDivElement | null;
          firstCard?.dispatchEvent(new Event("click"));
          log(t("log_batch_modal_ready", "Batch modal ready in {ms}ms", { ms: Math.round(performance.now() - t0) }));
          versionApplyBtn.onclick = async () => {
            const sel = document.querySelector<HTMLInputElement>('input[name="version-choice"]:checked');
            if (!sel || !currentKey) { log(t("log_please_select_version", "Please select a version"), true); return; }
            versionApplyBtn.disabled = true;
            try {
              const result = await invoke<string>("apply_selected_version", {
                gradlePath,
                source,
                projectId: currentKey,
                loader,
                selectedId: sel.value,
                cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
              });
              log(result);
              versionModal.style.display = "none";
            } catch (err) {
              log(t("log_apply_failed", "Apply failed: {err}", { err: String(err) }), true);
            } finally {
              versionApplyBtn.disabled = false;
            }
          };
          versionCancelBtn.onclick = () => { versionModal.style.display = "none"; };
          if (versionApplyAllBtn) versionApplyAllBtn.onclick = async () => {
            const pairs: Array<[string, string]> = [];
            Object.entries(selectedVersionByMod).forEach(([k, v]) => { if (v) pairs.push([k, v]); });
            if (pairs.length === 0) { log(t("log_please_select_version", "Please select a version"), true); return; }
            versionApplyAllBtn.disabled = true;
            try {
              const result = await invoke<string>("apply_selected_versions_batch", {
                gradlePath,
                source,
                selections: pairs,
                loader,
                cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
              });
              log(result);
              versionModal.style.display = "none";
            } catch (err) {
              log(t("log_apply_failed", "Apply failed: {err}", { err: String(err) }), true);
            } finally {
              versionApplyAllBtn.disabled = false;
            }
          };
        } catch (err) {
          log(t("log_error", "Error: {err}", { err: String(err) }), true);
        }
      }
    } catch (err) {
      log(t("log_error", "Error: {err}", { err: String(err) }), true);
    }
  });
}

async function waitDom() {
  if (document.readyState === "loading") {
    await new Promise<void>(resolve => document.addEventListener("DOMContentLoaded", () => resolve(), { once: true }));
  }
}

function isTauriReady() {
  const w = window as any;
  return !!(w.__TAURI__ && w.__TAURI__.core && typeof w.__TAURI__.core.invoke === "function");
}

async function waitTauri(maxMs = 4000) {
  const start = performance.now();
  while (!isTauriReady() && performance.now() - start < maxMs) {
    await new Promise(r => setTimeout(r, 50));
  }
}

async function init() {
  await waitDom();
  await waitTauri();
  loadState();
  langSelect.value = "en";
  await applyI18n("en");
  updateUIState();
  bindEvents();
  try {
    await listen("tauri://close-requested", async () => {
      if (isTauriReady() && (logOutput.textContent || "").trim()) {
        try { await invoke<string>("save_log", { content: logOutput.textContent || "" }); } catch {}
      }
    });
  } catch {}
  window.addEventListener("beforeunload", async () => {
    if (isTauriReady() && (logOutput.textContent || "").trim()) {
      try { await invoke<string>("save_log", { content: logOutput.textContent || "" }); } catch {}
    }
  });
}

init();
