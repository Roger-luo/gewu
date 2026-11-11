import type {
  GitHubAccount,
  GitHubBackend,
  GitHubPreferences,
  GitHubSettings,
  SignInMethod,
} from "./types";

/**
 * GitHub for the mock and tests. Gewu's app signs in through the device
 * flow with a fixed code, approved on the second poll. The "GitHub CLI" is
 * already signed in as {@link MOCK_CLI_LOGIN}. Avatars are drawn locally,
 * so nothing is fetched from GitHub.
 */
const STORAGE_KEY = "gewu.mock-github";
const METHOD_KEY = "gewu.mock-github-method";
const PREFS_KEY = "gewu.mock-github-prefs";

export const MOCK_LOGIN = "octocat";
export const MOCK_CLI_LOGIN = "hubot";
export const MOCK_CODE = "MOCK-1234";
export const MOCK_CLIENT_ID = "Ov23liMKKIvqy32L2nuS";

let polls = 0;
/** Polls before the mock approves the code; tests that cancel raise it. */
let approveAfter = 2;

export function holdMockApproval(hold: boolean): void {
  approveAfter = hold ? Number.POSITIVE_INFINITY : 2;
}

const method = (): SignInMethod => (localStorage.getItem(METHOD_KEY) === "gh" ? "gh" : "gewu");

/** The client ID and rehearsal choice; the method has its own key. */
function saved(): Pick<GitHubPreferences, "client_id" | "rehearse"> {
  try {
    const parsed = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") as Partial<GitHubPreferences>;
    return { client_id: parsed.client_id ?? null, rehearse: parsed.rehearse ?? false };
  } catch {
    return { client_id: null, rehearse: false };
  }
}

/** Whether a mock publish only rehearses. */
export function mockRehearsing(): boolean {
  return saved().rehearse;
}

const settings = (): GitHubSettings => ({
  method: method(),
  ...saved(),
  cli: { status: "signed_in", login: MOCK_CLI_LOGIN },
  default_client_id: MOCK_CLIENT_ID,
  sandbox: false,
});

export const mockGitHub: GitHubBackend = {
  async account() {
    if (method() === "gh") return accountFor(MOCK_CLI_LOGIN);
    return localStorage.getItem(STORAGE_KEY) !== null ? accountFor(MOCK_LOGIN) : null;
  },
  async startSignIn() {
    if (method() === "gh") return { kind: "token" };
    polls = 0;
    return {
      kind: "device",
      user_code: MOCK_CODE,
      verification_uri: "https://github.com/login/device",
      interval: 0,
      expires_in: 900,
    };
  },
  async pollSignIn() {
    polls += 1;
    if (polls < approveAfter) {
      return { status: "pending" };
    }
    localStorage.setItem(STORAGE_KEY, MOCK_LOGIN);
    return { status: "signed_in", account: accountFor(MOCK_LOGIN) };
  },
  async signIn(token: string) {
    return token.trim() ? accountFor(MOCK_CLI_LOGIN) : null;
  },
  async signOut() {
    if (method() === "gh") localStorage.removeItem(METHOD_KEY);
    else localStorage.removeItem(STORAGE_KEY);
  },
  async settings() {
    return settings();
  },
  async updateSettings(preferences: GitHubPreferences) {
    await this.chooseMethod(preferences.method);
    const client_id = preferences.client_id?.trim() || null;
    localStorage.setItem(PREFS_KEY, JSON.stringify({ client_id, rehearse: preferences.rehearse }));
    return settings();
  },
  async chooseMethod(next: SignInMethod) {
    if (next === "gh") localStorage.setItem(METHOD_KEY, "gh");
    else localStorage.removeItem(METHOD_KEY);
    return settings();
  },
};

/** Starts signed in to Gewu's app, as a returning reviewer would. */
export function signInMockGitHub(): void {
  localStorage.setItem(STORAGE_KEY, MOCK_LOGIN);
}

/** Signed out of Gewu's app, and back on it from the CLI. */
export function signOutMockGitHub(): void {
  localStorage.removeItem(STORAGE_KEY);
  localStorage.removeItem(METHOD_KEY);
  localStorage.removeItem(PREFS_KEY);
}

/** Who the mock repository's notes can `@`-mention. */
export const MOCK_CONTRIBUTORS = ["octocat", "grace", "linus", "ada"];

export function mockSignedIn(): boolean {
  return method() === "gh" || localStorage.getItem(STORAGE_KEY) !== null;
}

function accountFor(login: string): GitHubAccount {
  return { login, name: null, avatar_url: avatarFor(login), html_url: `https://github.com/${login}` };
}

/** An avatar that does not fetch anything. */
export function avatarFor(login: string): string {
  const initial = (login.trim()[0] ?? "?").toUpperCase();
  const hue = [...login].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="hsl(${hue} 42% 38%)"/><text x="32" y="42" text-anchor="middle" font-family="sans-serif" font-size="30" fill="white">${initial}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
