/** A GitHub account linked on this computer. The avatar is drawn locally. */

const STORAGE_KEY = "gewu.github";

export interface GitHubAccount {
  login: string;
  avatarUrl: string;
}

export function readAccount(): GitHubAccount | null {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw) as Partial<GitHubAccount>;
    if (!parsed.login || !parsed.avatarUrl) {
      return null;
    }
    return { login: parsed.login, avatarUrl: parsed.avatarUrl };
  } catch {
    return null;
  }
}

export function writeAccount(account: GitHubAccount | null): void {
  if (account) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(account));
  } else {
    localStorage.removeItem(STORAGE_KEY);
  }
}

/** An avatar that does not fetch anything from GitHub. */
export function avatarFor(login: string): string {
  const initial = (login.trim()[0] ?? "?").toUpperCase();
  const hue = [...login].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="hsl(${hue} 42% 38%)"/><text x="32" y="42" text-anchor="middle" font-family="sans-serif" font-size="30" fill="white">${initial}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
