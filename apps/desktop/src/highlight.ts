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
  swift: () => import("shiki/langs/swift.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  java: () => import("shiki/langs/java.mjs"),
  kotlin: () => import("shiki/langs/kotlin.mjs"),
  ruby: () => import("shiki/langs/ruby.mjs"),
  c: () => import("shiki/langs/c.mjs"),
  cpp: () => import("shiki/langs/cpp.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  xml: () => import("shiki/langs/xml.mjs"),
  ini: () => import("shiki/langs/ini.mjs"),
  properties: () => import("shiki/langs/properties.mjs"),
  dotenv: () => import("shiki/langs/dotenv.mjs"),
  make: () => import("shiki/langs/make.mjs"),
  dockerfile: () => import("shiki/langs/dockerfile.mjs"),
  diff: () => import("shiki/langs/diff.mjs"),
  typst: () => import("shiki/langs/typst.mjs"),
};

/** Shiki ships no gitignore grammar; these scopes color in the GitHub themes. */
const GITIGNORE: LanguageRegistration = {
  name: "gitignore",
  scopeName: "source.gitignore",
  repository: {},
  patterns: [
    { match: "^\\s*#.*", name: "comment.line.number-sign.gitignore" },
    { match: "^\\s*!.*", name: "keyword.operator.negation.gitignore" },
    { match: "\\*+", name: "constant.other.wildcard.gitignore" },
    { match: "/$", name: "punctuation.definition.directory.gitignore" },
  ],
};

const CUSTOM: Record<string, LanguageRegistration> = { gitignore: GITIGNORE };

const ALIASES: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  py: "python",
  md: "markdown",
  yml: "yaml",
  rs: "rust",
  bash: "shell",
  sh: "shell",
  zsh: "shell",
  makefile: "make",
};

export interface HighlightToken {
  text: string;
  color?: string;
}

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
  return key in LANG_LOADERS || key in CUSTOM ? key : null;
}

async function ensureLanguage(lang: string): Promise<HighlighterCore> {
  const highlighter = await getHighlighter();
  if (!loadedLangs.has(lang)) {
    const custom = CUSTOM[lang];
    if (custom) {
      await highlighter.loadLanguage(custom);
    } else {
      const mod = await LANG_LOADERS[lang]();
      const langs = Array.isArray(mod.default) ? mod.default : [mod.default];
      await highlighter.loadLanguage(...langs);
    }
    loadedLangs.add(lang);
  }
  return highlighter;
}

function theme(dark: boolean): string {
  return dark ? "github-dark-high-contrast" : "github-light-high-contrast";
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
  const highlighter = await ensureLanguage(lang);
  return highlighter.codeToHtml(code, { lang, theme: theme(dark) });
}

/** One token list per source line, so a diff cell can keep its own background. */
export async function highlightLines(
  code: string,
  language: string | null | undefined,
  dark: boolean,
): Promise<HighlightToken[][] | null> {
  const lang = resolveLang(language);
  if (!lang) {
    return null;
  }
  const highlighter = await ensureLanguage(lang);
  const { tokens } = highlighter.codeToTokens(code, { lang, theme: theme(dark) });
  return tokens.map((line) => line.map((token) => ({ text: token.content, color: token.color })));
}

export function prefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}
