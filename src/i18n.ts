let currentDict: Record<string, string> = {};

export function t(key: string, fallback: string, params?: Record<string, string | number>) {
    const raw = currentDict[key] || fallback;
    if (!params) return raw;
    return Object.keys(params).reduce((acc, k) => acc.replace(new RegExp(`\\{${k}\\}`, "g"), String(params[k])), raw);
}

export async function applyI18n(lang: string) {
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

export function currentDictRef() {
    return currentDict;
}

