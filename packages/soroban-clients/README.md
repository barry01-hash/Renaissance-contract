# Soroban TypeScript clients

This package contains versioned TypeScript bindings generated from the local
Soroban contract WASM files. Do not edit files below `src/` by hand.

## Regenerate after a contract change

1. Install Rust, the `wasm32v1-none` target, Node.js, and Stellar CLI 26.1.0.
2. From the repository root, run `npm run generate:contracts`.
3. Run `npm run typecheck:contracts`.
4. Review and commit all changed files under `packages/soroban-clients/src/`.

The generator builds and refreshes the `betting`, `rewards`, `player-nft`, and
`vault` modules. Generation uses local WASM, so it does not require deployed
contract IDs or network access after dependencies and tools are installed.
