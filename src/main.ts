import { invoke } from "@tauri-apps/api/core";
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
    log("❌ Tauri 未就绪：无法解析项目信息", true);
    return;
  }
  if (modeSelect.value === "batch") {
    return;
  }
  const source = sourceSelect.value;
  const projectId = projectIdInput.value.trim();
  const cfApiKey = cfApiKeyInput.value.trim();
  if (!projectId) {
    log("❌ 请先填写工程标识", true);
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
    log(`✅ 已解析可选项：版本 ${versions.length}，加载器 ${loaders.length}`);
  } catch (err) {
    log(`❌ 解析失败: ${err}`, true);
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
    log(`🔄 版本选择: ${ver} → 加载器候选: ${targetLoaders.length}，版本候选: ${targetVersions.length}`);
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
    log(`🔄 加载器选择: ${loader} → 版本候选: ${targetVersions.length}，加载器候选: ${targetLoaders.length}`);
  });
  if (browseBtn) browseBtn.addEventListener("click", async () => {
    if (!isTauriReady()) {
      log("❌ Tauri 未就绪：无法打开系统文件选择器", true);
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
      log(`Error selecting file: ${err}`, true);
    }
  });
  if (clearLogBtn) clearLogBtn.addEventListener("click", () => { logOutput.textContent = ""; });
  if (saveLogBtn) saveLogBtn.addEventListener("click", async () => {
    if (!isTauriReady()) { log("❌ Tauri 未就绪：无法保存日志", true); return; }
    try {
      const path = await invoke<string>("save_log", { content: logOutput.textContent || "" });
      log(`✅ 已保存日志: ${path}`);
    } catch (err) {
      log(`❌ 保存失败: ${err}`, true);
    }
  });
  if (updateBtn) updateBtn.addEventListener("click", async () => {
    if (!isTauriReady()) {
      log("❌ Tauri 未就绪：无法调用后端命令", true);
      return;
    }
    log("Running update...", false);
    const gradlePath = gradlePathInput.value.trim();
    const source = sourceSelect.value;
    const projectId = projectIdInput.value.trim();
    const isBatch = modeSelect.value === "batch";
    const mcVersion = isBatch ? mcVersionInput.value.trim() : mcVersionSelect.value;
    const loader = isBatch ? loaderInput.value.trim() : loaderSelect.value;
    const cfApiKey = cfApiKeyInput.value.trim();
    if (!gradlePath) {
      log("❌ Please select a build.gradle file.", true);
      return;
    }
    if (!isBatch && !projectId) {
      log("❌ Please enter a Project ID.", true);
      return;
    }
    if (isBatch && (!mcVersion || !loader)) {
      log("❌ 批量模式下需要填写版本和加载器", true);
      return;
    }
    if (source === "curseforge" && !cfApiKey) {
      log("⚠ Warning: No API Key provided. If not set in environment variables, this will fail.", false);
    }
    saveState();
    try {
      if (!isBatch) {
        const result = await invoke<string>("update_dependency", {
          gradlePath,
          projectId,
          mcVersion,
          loader,
          source,
          cfApiKey: cfApiKey || null,
        });
        log(result);
      } else {
        const items = batchItems.value
          .split(/\r?\n/)
          .map(s => s.trim())
          .filter(Boolean);
        if (items.length === 0) {
          log("❌ 请在批量模式下输入项目列表", true);
          return;
        }
        const result = await invoke<string>("update_dependencies_batch", {
          gradlePath,
          source,
          items,
          mcVersion,
          loader,
          cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
        });
        log(result);
      }
    } catch (err) {
      log(`❌ Error: ${err}`, true);
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
  await applyI18n(langSelect.value || navigator.language);
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
