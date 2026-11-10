import type { GitHubAccount, GitHubBackend } from "./types";

/**
 * GitHub for the mock and tests: any token signs in as `octocat`, and the
 * avatar is drawn locally, so nothing is fetched from GitHub.
 */
const STORAGE_KEY = "gewu.mock-github";

export const MOCK_LOGIN = "octocat";

export const mockGitHub: GitHubBackend = {
  async account() {
    return localStorage.getItem(STORAGE_KEY) ? accountFor(MOCK_LOGIN) : null;
  },
  async signIn(token: string) {
    if (!token.trim()) {
      return null;
    }
    localStorage.setItem(STORAGE_KEY, MOCK_LOGIN);
    return accountFor(MOCK_LOGIN);
  },
};

export function signOutMockGitHub(): void {
  localStorage.removeItem(STORAGE_KEY);
}

export function mockSignedIn(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== null;
}

function accountFor(login: string): GitHubAccount {
  return { login, name: null, avatar_url: avatarFor(login), html_url: `https://github.com/${login}` };
}

/** An avatar that does not fetch anything. */
function avatarFor(login: string): string {
  const initial = (login.trim()[0] ?? "?").toUpperCase();
  const hue = [...login].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="hsl(${hue} 42% 38%)"/><text x="32" y="42" text-anchor="middle" font-family="sans-serif" font-size="30" fill="white">${initial}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
