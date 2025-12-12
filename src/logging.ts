import {logOutput} from "./dom";

type LogLevel = "info" | "warn" | "error";

function appendLog(level: LogLevel, msg: string, append: boolean) {
    if (!append) {
        logOutput.innerHTML = "";
    }
    const line = document.createElement("div");
    const tag = document.createElement("span");
    if (level === "info") {
        tag.textContent = "[INFO] ";
        tag.style.color = "#4caf50";
    } else if (level === "warn") {
        tag.textContent = "[WARN] ";
        tag.style.color = "#ffca28";
    } else {
        tag.textContent = "[ERROR] ";
        tag.style.color = "#f44336";
    }
    const text = document.createElement("span");
    text.textContent = msg;
    line.appendChild(tag);
    line.appendChild(text);
    logOutput.appendChild(line);
    logOutput.scrollTop = logOutput.scrollHeight;
}

export function log(msg: string, isError = false, append = true) {
    appendLog(isError ? "error" : "info", msg, append);
}

export function logWarn(msg: string, append = true) {
    appendLog("warn", msg, append);
}

