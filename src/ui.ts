import {
    apiKeyGroup,
    batchGroup,
    fetchOptionsBtn,
    loaderInputGroup,
    loaderSelectGroup,
    mcVersionInputGroup,
    mcVersionSelectGroup,
    modeSelect,
    projectGroup,
    projectLabel
} from "./dom";
import {currentDictRef} from "./i18n";

export function updateUIState() {
    const currentDict = currentDictRef();
    const source = (document.getElementById("source-select") as HTMLSelectElement).value;
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

export function setOptions(select: HTMLSelectElement, values: string[]) {
    select.innerHTML = "";
    values.forEach(v => {
        const opt = document.createElement("option");
        opt.value = v;
        opt.textContent = v;
        select.appendChild(opt);
    });
}

