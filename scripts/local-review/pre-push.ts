import { spawn, spawnSync } from "node:child_process";
import { appendFile, mkdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

const ZERO_OID = /^0+$/;
const OID = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/i;
const SAFE_BRANCH = /^[A-Za-z0-9._/-]+$/;
const LOG_DIR = join(process.env.LOCALAPPDATA ?? join(homedir(), "AppData", "Local"), "DecisionNarrative", "local-review");
const LOG_FILE = join(LOG_DIR, "review.log");

async function log(message: string): Promise<void> {
  try {
    await mkdir(LOG_DIR, { recursive: true });
    await appendFile(LOG_FILE, `[${new Date().toISOString()}] ${message}\n`, "utf8");
  } catch {
    // Logging failures must never make a user's push fail.
  }
}

function cleanEnv(): NodeJS.ProcessEnv {
  const env = { ...process.env };
  delete env.GIT_DIR;
  delete env.GIT_WORK_TREE;
  delete env.GIT_COMMON_DIR;
  delete env.GIT_INDEX_FILE;
  delete env.GIT_OBJECT_DIRECTORY;
  delete env.GIT_ALTERNATE_OBJECT_DIRECTORIES;
  delete env.GIT_PREFIX;
  delete env.GIT_SUPER_PREFIX;
  return env;
}

function safeBranchRef(ref: string): string | undefined {
  if (!ref.startsWith("refs/heads/")) return;
  const branch = ref.slice("refs/heads/".length);
  if (!branch || !SAFE_BRANCH.test(branch) || branch.startsWith("-") || branch.endsWith("/")) return;
  if (branch.includes("..") || branch.includes("//") || branch.includes("@{") || branch.endsWith(".")) return;
  if (branch.split("/").some((part) => !part || part.startsWith(".") || part.endsWith(".lock"))) return;
  return branch;
}

function trustedRemote(url: string): { owner: string; repo: string; canonicalUrl: string } | undefined {
  const match = /^https:\/\/github\.com\/([A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?)\/([A-Za-z0-9_.-]+?)(?:\.git)?\/?$/i.exec(url);
  if (!match || match[2] === "." || match[2] === "..") return;
  const repo = match[2].replace(/\.git$/i, "");
  if (!repo || repo.startsWith("-") || repo.includes("..")) return;
  return { owner: match[1], repo, canonicalUrl: `https://github.com/${match[1]}/${repo}.git` };
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk));
  return Buffer.concat(chunks).toString("utf8");
}

function discoverRepoRoot(): string | undefined {
  const result = spawnSync("git", ["rev-parse", "--show-toplevel"], {
    cwd: process.cwd(),
    env: process.env,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.status !== 0 || result.error) return;
  return result.stdout.trim() || undefined;
}

async function main(): Promise<void> {
  // Git pre-push passes only <remote-name> and <remote-url>; derive the repo root.
  const [, , remoteName, remoteUrl] = process.argv;
  const repoRoot = discoverRepoRoot();
  if (!repoRoot || !remoteName || !remoteUrl || remoteName.length > 200) {
    await log("pre-push could not resolve repository root or remote arguments; no review tasks started");
    return;
  }

  const remote = trustedRemote(remoteUrl);
  if (!remote) {
    await log(`ignored unsupported remote URL (${JSON.stringify(remoteName)}); expected https://github.com/OWNER/REPO`);
    return;
  }

  const lines = (await readStdin()).split(/\r?\n/).filter(Boolean);
  for (const line of lines) {
    const fields = line.trim().split(/\s+/);
    if (fields.length !== 4) {
      await log("ignored malformed pre-push ref line");
      continue;
    }
    const [localRef, localOid, remoteRef, remoteOid] = fields;
    const branch = safeBranchRef(remoteRef);
    if (!branch || !OID.test(localOid) || ZERO_OID.test(localOid) || (!OID.test(remoteOid) && !ZERO_OID.test(remoteOid))) continue;

    const task = {
      repoRoot,
      sourceRemote: remote.canonicalUrl,
      sourceOwner: remote.owner,
      sourceRepo: remote.repo,
      remoteName,
      branch,
      commit: localOid.toLowerCase(),
      remoteBefore: remoteOid.toLowerCase(),
      localRef,
    };
    try {
      const child = spawn(process.execPath, [join(import.meta.dir, "worker.ts"), JSON.stringify(task)], {
        cwd: repoRoot,
        detached: true,
        stdio: "ignore",
        windowsHide: true,
        env: cleanEnv(),
      });
      child.once("error", (error) => void log(`failed to start worker for ${branch}@${localOid}: ${error.message}`));
      child.unref();
      await log(`queued local review for ${branch}@${localOid} (${JSON.stringify(localRef)}, remote ${JSON.stringify(remoteName)})`);
    } catch (error) {
      await log(`failed to queue ${branch}@${localOid}: ${String(error)}`);
    }
  }
}

main().catch(async (error) => {
  await log(`pre-push hook error (push is not blocked): ${String(error)}`);
});
