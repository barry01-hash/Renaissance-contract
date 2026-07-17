import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outputRoot = join(root, "packages", "soroban-clients", "src");

const contracts = [
  ["betting", "renaissance-betting", "renaissance_betting.wasm"],
  ["rewards", "renaissance-rewards", "renaissance_rewards.wasm"],
  ["player-nft", "renaissance-player-nft", "renaissance_player_nft.wasm"],
  ["vault", "renaissance-vault", "renaissance_vault.wasm"],
];

mkdirSync(outputRoot, { recursive: true });

for (const [moduleName, packageName, wasmName] of contracts) {
  execFileSync(
    "stellar",
    [
      "contract",
      "build",
      "--package",
      packageName,
      "--locked",
      "--optimize=false",
    ],
    { cwd: root, stdio: "inherit" },
  );

  execFileSync(
    "stellar",
    [
      "contract",
      "bindings",
      "typescript",
      "--wasm",
      join(root, "target", "wasm32v1-none", "release", wasmName),
      "--output-dir",
      join(outputRoot, moduleName),
      "--overwrite",
    ],
    { cwd: root, stdio: "inherit" },
  );
}
