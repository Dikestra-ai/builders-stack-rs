# Known issues

One upstream rough edge remains in this template: Next.js/Turbopack cannot
follow Bun global-store links. The earlier macOS Gatekeeper and unbounded
removal stalls are resolved or bounded by Worktree Zero 0.1.19; the retained
process bounds still fail closed when the machine cannot prove safety.

## Turbopack fails under Bun's isolated linker + global store

**What breaks:** `next build` (Turbopack, the default since Next 15) fails with `Symlink … points out of the filesystem root`, on a local machine and on a fresh GitHub Actions runner alike.

**Why:** `bunfig.toml` here sets `linker = "isolated"` and `globalStore = true` (see the file's own comment) — Bun installs every package once into a shared store under the home directory (`~/.bun/install/cache/links/...`) and symlinks each checkout's `node_modules` to it, so parallel worktrees stay isolated without copying gigabytes of dependencies into each one. Turbopack refuses those dependency symlinks as outside its filesystem root. Setting `turbopack.root` to the home directory — the common ancestor of the checkout and store — was tried and failed with the same React and `@swc/helpers` errors on GitHub Actions; it is not a workaround. Tracked upstream: [vercel/next.js#94432](https://github.com/vercel/next.js/issues/94432).

**Workaround shipped:** each app's canonical `build` script (`apps/web`, `apps/blog`, `apps/landing`) runs:

```sh
next build --webpack
```

Webpack follows the shared-store links correctly. `scripts/check-agent-readability.ts` invokes `bun run build` in each app, so the PR gate builds, starts, and crawls all three apps through this same production path.

**Remove this when:** vercel/next.js#94432 is fixed upstream, or this repo stops using Bun's isolated linker + global store (drop `linker = "isolated"` / `globalStore = true` from `bunfig.toml`, and each app gets its own uncollapsed `node_modules` instead).

## Removal can refuse when `lsof` cannot finish

**What breaks:** on a heavily loaded or unhealthy machine, removing a Worktree
Zero-managed worktree can refuse after its bounded liveness proof instead of
finishing in under a second. The refusal is intentional: a timeout is not
evidence that no process is using the checkout.

**Why:** both Worktree Zero and Builders Stack inspect open working directories
through `lsof`. A system-wide `lsof` sweep can be slow even when wt0 itself is
healthy.

Worktree Zero 0.1.19 bounds its own `lsof` calls at 20 seconds by default and
reports a distinct safety refusal. The repository wrapper keeps its independent
process-group bound at 30 seconds because its pre-remove hook must remain safe
even when another wt0 operation holds a lock.

The original six-minute first-launch failure is closed: 0.1.19's macOS binaries
are Developer ID-signed and notarized. The launcher still bounds every
`--version` probe and prefers a satisfying PATH installation, so an unhealthy
toolchain or older cached binary cannot turn the wrapper into another
unbounded wait.

**What to do:** retry after the machine settles. If the sweep remains slow,
inspect the machine before raising `WT0_LSOF_TIMEOUT` or
`BUILDERS_STACK_LIVE_CHECK_TIMEOUT_SECONDS`; never bypass the refusal. The
wrapper uses 0.1.19's cheap `wt0 list --json` ownership fields and falls back
to the heavier fleet probe only for an older binary.

**Remove this entry when:** real fleet measurements show the liveness proof is
consistently bounded below the normal interactive threshold; keep the
fail-closed process bounds regardless.

See [builders-stack#53](https://github.com/lonormaly/builders-stack/issues/53) for the original report and reproduction.
