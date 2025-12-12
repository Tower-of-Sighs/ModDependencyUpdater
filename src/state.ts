import {
    gradlePathInput,
    sourceSelect,
    projectIdInput,
    mcVersionSelect,
    loaderSelect,
    cfApiKeyInput,
    langSelect,
    modeSelect,
    cacheToggle
} from "./dom";
import {updateUIState} from "./ui";

export function loadState() {
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
        if (Object.prototype.hasOwnProperty.call(state, "cacheVersions")) cacheToggle.checked = !!state.cacheVersions;
        updateUIState();
    }
}

export function saveState() {
    const state = {
        gradlePath: gradlePathInput.value,
        source: sourceSelect.value,
        projectId: projectIdInput.value,
        mcVersion: mcVersionSelect.value,
        loader: loaderSelect.value,
        cfApiKey: cfApiKeyInput.value,
        lang: langSelect.value,
        mode: modeSelect.value,
        cacheVersions: cacheToggle.checked,
    };
    localStorage.setItem("mod-dep-updater-state", JSON.stringify(state));
}
