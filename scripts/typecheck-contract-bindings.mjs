import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const modules = ["betting", "rewards", "player-nft", "vault"];
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

for (const moduleName of modules) {
  const cwd = join(root, "packages", "soroban-clients", "src", moduleName);
  execFileSync(npm, ["install", "--ignore-scripts"], { cwd, stdio: "inherit" });
  execFileSync(npm, ["exec", "--", "tsc", "--noEmit"], {
    cwd,
    stdio: "inherit",
  });
}
