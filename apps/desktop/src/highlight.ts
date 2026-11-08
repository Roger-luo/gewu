import {
  createHighlighterCore,
  type HighlighterCore,
  type LanguageRegistration,
} from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

type LangModule = { default: LanguageRegistration[] | LanguageRegistration };

const LANG_LOADERS: Record<string, () => Promise<LangModule>> = {
  rust: () => import("shiki/langs/rust.mjs"),
  typescript: () => import("shiki/langs/typescript.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  toml: () => import("shiki/langs/toml.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  markdown: () => import("shiki/langs/markdown.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
  shell: () => import("shiki/langs/shellscript.mjs"),
  bash: () => import("shiki/langs/shellscript.mjs"),
  sh: () => import("shiki/langs/shellscript.mjs"),
  swift: () => import("shiki/langs/swift.mjs"),
};

const ALIASES: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  py: "python",
  md: "markdown",
  yml: "yaml",
  rs: "rust",
};

let highlighterPromise: Promise<HighlighterCore> | null = null;
const loadedLangs = new Set<string>();

async function getHighlighter(): Promise<HighlighterCore> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighterCore({
      themes: [
        import("shiki/themes/github-light-high-contrast.mjs"),
        import("shiki/themes/github-dark-high-contrast.mjs"),
      ],
      langs: [],
      engine: createJavaScriptRegexEngine(),
    });
  }
  return highlighterPromise;
}

function resolveLang(language: string | null | undefined): string | null {
  if (!language) {
    return null;
  }
  const key = ALIASES[language.toLowerCase()] ?? language.toLowerCase();
  return key in LANG_LOADERS ? key : null;
}

export async function highlightCode(
  code: string,
  language: string | null | undefined,
  dark: boolean,
): Promise<string | null> {
  const lang = resolveLang(language);
  if (!lang) {
    return null;
  }
  const highlighter = await getHighlighter();
  if (!loadedLangs.has(lang)) {
    const mod = await LANG_LOADERS[lang]();
    const langs = Array.isArray(mod.default) ? mod.default : [mod.default];
    await highlighter.loadLanguage(...langs);
    loadedLangs.add(lang);
  }
  return highlighter.codeToHtml(code, {
    lang,
    theme: dark ? "github-dark-high-contrast" : "github-light-high-contrast",
  });
}

export function prefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}
