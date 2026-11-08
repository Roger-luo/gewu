import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export async function gitHeadAndDirty(cwd: string): Promise<{
  head: string | null;
  dirty: boolean;
}> {
  try {
    const { stdout: headOut } = await execFileAsync("git", ["rev-parse", "HEAD"], { cwd });
    const head = headOut.trim() || null;
    const { stdout: statusOut } = await execFileAsync(
      "git",
      ["status", "--porcelain"],
      { cwd },
    );
    return { head, dirty: statusOut.trim().length > 0 };
  } catch {
    return { head: null, dirty: false };
  }
}
