import {open} from "@tauri-apps/plugin-dialog";
import {openPath as openExternal} from "@tauri-apps/plugin-opener";
import {convertFileSrc, invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {applyI18n, t} from "./i18n";
import {log, logWarn} from "./logging";
import {loadState, saveState} from "./state";
import {setOptions, updateUIState} from "./ui";
import {
    batchItems,
    browseBtn,
    cacheToggle,
    clearCacheBtn,
    clearLogBtn,
    gradlePathInput,
    langSelect,
    loaderInput,
    loaderSelect,
    logOutput,
    mainView,
    mcVersionInput,
    mcVersionSelect,
    modeSelect,
    modStrip,
    openLogBtn,
    projectIdInput,
    refreshCacheBtn,
    saveLogBtn,
    sourceSelect,
    toolAwInputMapping,
    toolAwInputMappingGroup,
    toolAwOutputMapping,
    toolAwOutputMappingGroup,
    toolAwOutputName,
    toolAwOutputNameGroup,
    toolAwTargetMapping,
    toolAwTargetMappingGroup,
    toolBackBtn,
    toolBrowseBtn,
    toolClearLogBtn,
    toolConvertBtn,
    toolDirection,
    toolFailures,
    toolInputPath,
    toolLogOutput,
    toolMcVersion,
    toolOpenLogBtn,
    toolSaveLogBtn,
    toolsBtn,
    toolStats,
    toolView,
    updateBtn,
    versionApplyAllBtn,
    versionApplyBtn,
    versionCancelBtn,
    versionList,
    versionModal,
} from "./dom";
import {ProjectOptions} from "./types";

let versionToLoaders: Record<string, string[]> = {};
let loaderToVersions: Record<string, string[]> = {};
let allVersions: string[] = [];
let allLoaders: string[] = [];

function isTauriReady() {
    const w = window as any;
    return !!(w.__TAURI__ && w.__TAURI__.core && typeof w.__TAURI__.core.invoke === "function");
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
    const cfApiKey = (document.getElementById("cf-api-key") as HTMLInputElement).value.trim();
    if (!projectId) {
        log(t("log_missing_project_id", "Please enter a Project ID."), true);
        return;
    }
    try {
        const result = await invoke<ProjectOptions>("get_project_options", {
            source,
            projectId,
            cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
            useCache: cacheToggle.checked,
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
        log(t("log_parsed_options", "Parsed options: versions {v}, loaders {l}", {
            v: versions.length,
            l: loaders.length
        }));
    } catch (err) {
        log(t("log_parse_failed", "Parse failed: {err}", {err: String(err)}), true);
    }
}

function clearOptions() {
    setOptions(mcVersionSelect, []);
    setOptions(loaderSelect, []);
    versionToLoaders = {};
    loaderToVersions = {};
}

export function bindEvents() {
    if (sourceSelect) sourceSelect.addEventListener("change", updateUIState);
    if (sourceSelect) sourceSelect.addEventListener("change", clearOptions);
    if (modeSelect) modeSelect.addEventListener("change", () => {
        updateUIState();
        versionModal.style.display = "none";
        modStrip.innerHTML = "";
        versionList.innerHTML = "";
    });
    if (langSelect) langSelect.addEventListener("change", () => {
        applyI18n(langSelect.value).then(() => {
            updateUIState();
            saveState();
        });
    });
    if (projectIdInput) projectIdInput.addEventListener("input", clearOptions);
    if (document.getElementById("fetch-options")) document.getElementById("fetch-options")!.addEventListener("click", fetchProjectOptions);
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
        log(t("log_version_change", "Version: {ver} → loaders {lc}, versions {vc}", {
            ver,
            lc: targetLoaders.length,
            vc: targetVersions.length
        }));
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
        log(t("log_loader_change", "Loader changed: {loader}", {loader}));
    });
    if (browseBtn) browseBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            log(t("log_tauri_not_ready_open", "Tauri not ready: cannot open file dialog"), true);
            return;
        }
        try {
            const selected = await open({
                multiple: false,
                filters: [
                    {
                        name: "Gradle Files",
                        extensions: ["gradle"],
                    },
                    {
                        name: "All Files",
                        extensions: ["*"],
                    },
                ],
            });
            if (selected) {
                gradlePathInput.value = selected as string;
                saveState();
                clearOptions();
            }
        } catch (err) {
            log(t("log_select_file_error", "Error selecting file: {err}", {err: String(err)}), true);
        }
    });
    if (clearLogBtn) clearLogBtn.addEventListener("click", () => {
        logOutput.textContent = "";
    });
    if (saveLogBtn) saveLogBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            log(t("log_tauri_not_ready_save_log", "Tauri not ready: cannot save log"), true);
            return;
        }
        try {
            const path = await invoke<string>("save_log", {content: logOutput.textContent || ""});
            log(t("log_saved_log", "Saved log: {path}", {path}));
        } catch (err) {
            log(t("log_save_failed", "Save failed: {err}", {err: String(err)}), true);
        }
    });
    if (openLogBtn) openLogBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            log(t("log_tauri_not_ready_invoke", "Tauri not ready: cannot invoke backend commands"), true);
            return;
        }
        try {
            const dir = await invoke<string>("get_log_dir");
            await openExternal(dir);
            log(t("log_open_logs_dir", "Opened logs folder: {dir}", {dir}));
        } catch (err) {
            log(t("log_open_logs_failed", "Open logs folder failed: {err}", {err: String(err)}), true);
        }
    });
    if (clearCacheBtn) clearCacheBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            log(t("log_tauri_not_ready_invoke", "Tauri not ready: cannot invoke backend commands"), true);
            return;
        }
        try {
            await invoke("clear_all_caches");
            log(t("log_cache_cleared", "Cache cleared."));
        } catch (err) {
            log(t("log_cache_clear_failed", "Clear cache failed: {err}", {err: String(err)}), true);
        }
    });
    if (refreshCacheBtn) refreshCacheBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            log(t("log_tauri_not_ready_invoke", "Tauri not ready: cannot invoke backend commands"), true);
            return;
        }
        try {
            await invoke("refresh_mojang_cache");
            log(t("log_cache_refreshed", "Cache refresh triggered."));
        } catch (err) {
            log(t("log_cache_refresh_failed", "Cache refresh failed: {err}", {err: String(err)}), true);
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
        const cfApiKey = (document.getElementById("cf-api-key") as HTMLInputElement).value.trim();
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
            logWarn(t("log_no_api_key_warning", "Warning: No API Key provided. If not set in environment variables, this will fail."));
        }
        saveState();
        try {
            if (!isBatch) {
                modStrip.innerHTML = "";
                if (versionApplyAllBtn) versionApplyAllBtn.style.display = "none";
                if (versionApplyBtn) versionApplyBtn.style.display = "";
                const res = await invoke<{ choices: { id: string; label: string; kind: string }[] }>("list_versions", {
                    source,
                    projectId,
                    mcVersion,
                    loader,
                    cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
                    useCache: cacheToggle.checked,
                });
                const choices = res.choices || [];
                if (choices.length === 0) {
                    log(t("log_no_versions_found", "No matching versions found"), true);
                    return;
                }
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
                    if (!sel) {
                        log(t("log_please_select_version", "Please select a version"), true);
                        return;
                    }
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
                        log(t("log_apply_failed", "Apply failed: {err}", {err: String(err)}), true);
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
                versionApplyBtn.onclick = applyHandler;
                versionCancelBtn.onclick = cancelHandler;
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
                    const briefs = await invoke<{
                        mods: { key: string; name: string; icon: string; icon_data: string }[]
                    }>("get_batch_mod_briefs", {
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
                            radio.checked = selectedVersionByMod[key] ? selectedVersionByMod[key] === c.id : idx === 0;
                            radio.addEventListener("change", () => {
                                selectedVersionByMod[key] = c.id;
                            });
                            const label = document.createElement("label");
                            label.htmlFor = id;
                            label.textContent = c.label;
                            item.appendChild(radio);
                            item.appendChild(label);
                            versionList.appendChild(item);
                        });
                        if (!selectedVersionByMod[key] && choices.length > 0) {
                            selectedVersionByMod[key] = choices[0].id;
                        }
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
                                    const res = await invoke<{
                                        choices: { id: string; label: string; kind: string }[]
                                    }>("list_versions", {
                                        source,
                                        projectId: m.key,
                                        mcVersion,
                                        loader,
                                        cfApiKey: source === "curseforge" ? (cfApiKey || null) : null,
                                        useCache: cacheToggle.checked,
                                    });
                                    choices = res.choices || [];
                                    versionsCache[m.key] = choices;
                                    log(t("log_versions_loaded", "Versions for {key} loaded in {ms}ms (count {n})", {
                                        key: m.key,
                                        ms: Math.round(performance.now() - t1),
                                        n: choices.length,
                                    }));
                                } catch (err) {
                                    log(t("log_parse_failed", "Parse failed: {err}", {err: String(err)}), true);
                                    return;
                                }
                            }
                            renderChoices(m.key, choices);
                        });
                        modStrip.appendChild(card);
                        if (idx === 0) {
                            card.classList.add("active");
                            currentKey = m.key;
                        }
                    });
                    versionModal.style.display = "flex";
                    if (versionApplyBtn) versionApplyBtn.style.display = "none";
                    if (versionApplyAllBtn) versionApplyAllBtn.style.display = "";
                    log(t("log_batch_modal_show", "Showing batch modal: {n} mods", {n: mods.length}));
                    const firstCard = modStrip.querySelector(".mod-card") as HTMLDivElement | null;
                    firstCard?.dispatchEvent(new Event("click"));
                    log(t("log_batch_modal_ready", "Batch modal ready in {ms}ms", {ms: Math.round(performance.now() - t0)}));
                    versionApplyBtn.onclick = async () => {
                        const sel = document.querySelector<HTMLInputElement>('input[name="version-choice"]:checked');
                        if (!sel || !currentKey) {
                            log(t("log_please_select_version", "Please select a version"), true);
                            return;
                        }
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
                            log(t("log_apply_failed", "Apply failed: {err}", {err: String(err)}), true);
                        } finally {
                            versionApplyBtn.disabled = false;
                        }
                    };
                    versionCancelBtn.onclick = () => {
                        versionModal.style.display = "none";
                    };
                    if (versionApplyAllBtn) versionApplyAllBtn.onclick = async () => {
                        const pairs: Array<[string, string]> = [];
                        Object.entries(selectedVersionByMod).forEach(([k, v]) => {
                            if (v) pairs.push([k, v]);
                        });
                        if (pairs.length === 0) {
                            log(t("log_please_select_version", "Please select a version"), true);
                            return;
                        }
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
                            log(t("log_apply_failed", "Apply failed: {err}", {err: String(err)}), true);
                        } finally {
                            versionApplyAllBtn.disabled = false;
                        }
                    };
                } catch (err) {
                    log(t("log_error", "Error: {err}", {err: String(err)}), true);
                }
            }
        } catch (err) {
            log(t("log_error", "Error: {err}", {err: String(err)}), true);
        }
    });
    if (toolsBtn) toolsBtn.addEventListener("click", () => {
        mainView.style.display = "none";
        toolView.style.display = "block";
        toolLogOutput.textContent = "";
    });
    if (toolBackBtn) toolBackBtn.addEventListener("click", () => {
        toolView.style.display = "none";
        mainView.style.display = "block";
        logOutput.textContent = "";
    });
    if (toolDirection) toolDirection.addEventListener("change", () => {
        const dir = toolDirection.value;
        toolAwInputMappingGroup.style.display = dir === "aw_to_at" || dir === "aw_to_aw" ? "block" : "none";
        toolAwTargetMappingGroup.style.display = dir === "at_to_aw" ? "block" : "none";
        toolAwOutputMappingGroup.style.display = dir === "aw_to_aw" ? "block" : "none";
        toolAwOutputNameGroup.style.display = dir === "at_to_aw" || dir === "aw_to_aw" ? "block" : "none";
    });
    if (toolBrowseBtn) toolBrowseBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            toolLogOutput.textContent = "Tauri not ready: cannot open file dialog";
            return;
        }
        try {
            const selected = await open({
                multiple: false,
                filters: [
                    {
                        name: "Access Widener/Transformer",
                        extensions: ["accesswidener", "cfg", "*"],
                    },
                ],
            });
            if (selected) {
                toolInputPath.value = selected as string;
            }
        } catch (err) {
            toolLogOutput.textContent = `Error selecting file: ${String(err)}`;
        }
    });
    if (toolClearLogBtn) toolClearLogBtn.addEventListener("click", () => {
        toolLogOutput.textContent = "";
    });
    if (toolSaveLogBtn) toolSaveLogBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            toolLogOutput.textContent = "Tauri not ready: cannot save log";
            return;
        }
        try {
            const path = await invoke<string>("save_log", {content: toolLogOutput.textContent || ""});
            toolLogOutput.textContent = `Saved log: ${path}`;
        } catch (err) {
            toolLogOutput.textContent = `Save failed: ${String(err)}`;
        }
    });
    if (toolOpenLogBtn) toolOpenLogBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            toolLogOutput.textContent = "Tauri not ready: cannot invoke backend commands";
            return;
        }
        try {
            const dir = await invoke<string>("get_log_dir");
            await openExternal(dir);
            toolLogOutput.textContent = `Opened logs folder: ${dir}`;
        } catch (err) {
            toolLogOutput.textContent = `Open logs folder failed: ${String(err)}`;
        }
    });
    if (toolConvertBtn) toolConvertBtn.addEventListener("click", async () => {
        if (!isTauriReady()) {
            toolLogOutput.textContent = "Tauri not ready: cannot invoke backend commands";
            return;
        }
        toolStats.textContent = "";
        toolFailures.innerHTML = "";
        const inputPath = toolInputPath.value.trim();
        const mcVersion = toolMcVersion.value.trim();
        const dir = toolDirection.value;
        if (!inputPath) {
            toolLogOutput.textContent = "Please select an input file.";
            return;
        }
        if (!mcVersion) {
            toolLogOutput.textContent = "Please enter Minecraft version.";
            return;
        }
        toolConvertBtn.disabled = true;
        toolLogOutput.textContent = "Converting...";
        try {
            const res = await invoke<{
                outputPath: string;
                converted: number;
                failed: number;
                failures: string[]
            }>("convert_aw_at", {
                inputPath,
                mcVersion,
                direction: dir,
                inputMapping: dir === "aw_to_at" || dir === "aw_to_aw" ? toolAwInputMapping.value : null,
                outputMapping:
                    dir === "at_to_aw" ? toolAwTargetMapping.value :
                        dir === "aw_to_aw" ? toolAwOutputMapping.value : null,
                awOutputName: (dir === "at_to_aw" || dir === "aw_to_aw") ? (toolAwOutputName.value.trim() || null) : null,
            });
            toolLogOutput.textContent = `Converted: ${res.outputPath}`;
            toolStats.textContent = `Converted: ${res.converted}, Failed: ${res.failed}`;
            if (Array.isArray(res.failures)) {
                const total = res.failures.length;
                const threshold = 200;
                const list = document.createElement("ul");
                const items = total > threshold ? res.failures.slice(0, threshold) : res.failures;
                items.forEach(msg => {
                    const li = document.createElement("li");
                    li.textContent = msg;
                    list.appendChild(li);
                });
                toolFailures.appendChild(list);
                if (total > threshold) {
                    const btn = document.createElement("button");
                    btn.className = "secondary";
                    btn.textContent = `Show All (${total})`;
                    btn.addEventListener("click", () => {
                        toolFailures.innerHTML = "";
                        const fullList = document.createElement("ul");
                        res.failures.forEach(msg => {
                            const li = document.createElement("li");
                            li.textContent = msg;
                            fullList.appendChild(li);
                        });
                        const collapseBtn = document.createElement("button");
                        collapseBtn.className = "secondary";
                        collapseBtn.textContent = "Collapse";
                        collapseBtn.addEventListener("click", () => {
                            toolFailures.innerHTML = "";
                            toolFailures.appendChild(list);
                            toolFailures.appendChild(btn);
                        });
                        toolFailures.appendChild(fullList);
                        toolFailures.appendChild(collapseBtn);
                    });
                    toolFailures.appendChild(btn);
                }
            }
        } catch (err) {
            toolLogOutput.textContent = `Convert failed: ${String(err)}`;
            toolStats.textContent = "";
            toolFailures.innerHTML = "";
        } finally {
            toolConvertBtn.disabled = false;
        }
    });
}

export async function waitDom() {
    if (document.readyState === "loading") {
        await new Promise<void>(resolve => document.addEventListener("DOMContentLoaded", () => resolve(), {once: true}));
    }
}

export async function waitTauri(maxMs = 4000) {
    const start = performance.now();
    while (!isTauriReady() && performance.now() - start < maxMs) {
        await new Promise(r => setTimeout(r, 50));
    }
}

export async function initApp() {
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
                try {
                    await invoke<string>("save_log", {content: logOutput.textContent || ""});
                } catch {
                }
            }
        });
    } catch {
    }
    window.addEventListener("beforeunload", async () => {
        if (isTauriReady() && (logOutput.textContent || "").trim()) {
            try {
                await invoke<string>("save_log", {content: logOutput.textContent || ""});
            } catch {
            }
        }
    });
}
