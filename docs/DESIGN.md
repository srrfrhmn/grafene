# Grafene — Design Document

| Field   | Value                         |
|---------|-------------------------------|
| Status  | Draft 1                       |
| Date    | 2026-11-07                    |
| Owner   | srrfrhmn                      |
| Style   | ASD-STE100-inspired           |

---

## 1. Purpose

This document gives the design for Grafene.
Grafene is a code review platform for stacked pull requests (PRs).
Grafene includes an AI reviewer.
Graphite is the main reference product.

Grafene has three priorities, in this sequence:

1. Speed.
2. Low resource use ("liteness").
3. Review quality with low noise.

Grafene uses Rust for all core components.

---

## 2. Scope

### 2.1 Goals

- Give a fast CLI to create, sync, and submit stacks of PRs.
- Give a fast web UI to review PRs and stacks.
- Give an AI reviewer that finds real defects and makes few false reports.
- Run as one static binary. Use SQLite by default.
- Support self-hosted and local-only operation.
- Work with GitHub first. Add GitLab and Forgejo later.

### 2.2 Non-goals

- Grafene is not a Git host. It does not store the canonical repository.
- Grafene is not a CI system. It reads CI results. It does not run builds.
- Grafene does not train models.
- Version 1 does not support Mercurial or Perforce.

---

## 3. Terms

| Term          | Meaning                                                              |
|---------------|----------------------------------------------------------------------|
| Stack         | An ordered chain of branches. Each branch depends on the branch below it. |
| Trunk         | The base branch of a stack (for example, `main`).                    |
| Restack       | Rebase each branch of a stack onto its new parent.                   |
| Revision      | One pushed version of a PR.                                          |
| Interdiff     | The diff between two revisions of the same PR.                       |
| Finding       | One issue that the AI reviewer or a rule reports.                    |
| Forge         | The Git host (GitHub, GitLab, Forgejo).                              |
| Hunk hash     | A content hash of one diff hunk plus its context lines.              |

---

## 4. Baseline features (parity with Graphite)

Grafene must have these features before release 1.0.

| ID   | Feature                    | Description                                                   |
|------|----------------------------|---------------------------------------------------------------|
| B-01 | Stack CLI                  | Create, move, fold, split, reorder, and restack branches.     |
| B-02 | Stack submit               | Push all branches. Open or update one PR for each branch.     |
| B-03 | Stack sync                 | Pull trunk. Delete merged branches. Restack the remainder.    |
| B-04 | PR inbox                   | Show PRs that need action from the user, in sections.         |
| B-05 | Stack view                 | Show all PRs of a stack on one page, with status.             |
| B-06 | Diff review                | Split and unified diffs. Inline comments. Suggestions.        |
| B-07 | AI review                  | Automatic AI comments on each revision.                       |
| B-08 | Merge queue                | Stack-aware queue. Merge a full stack in one operation.       |
| B-09 | Notifications              | Web, email digest, Slack, and webhooks.                       |
| B-10 | Insights                   | Review time, PR size, and throughput for each team.           |

---

## 5. Distinct features

These features make Grafene different from other AI review platforms.
Each feature has an ID. The milestones (section 13) refer to these IDs.

### D-01 Local-first review

- The CLI runs the full review engine on the local machine: `grf review`.
- The local engine and the server engine are the same crate.
- The user can use a local model (OpenAI-compatible endpoint, for example llama.cpp).
- Thus, the user can get AI review before the push, and without network access.
- Findings from a local run go to the server with the PR. The server does not repeat that work.

### D-02 Rebase-stable findings

- Grafene keys each review result by hunk hash, rule version, and model ID.
- A restack changes commit IDs, but it does not change most hunks.
- Grafene does not review an unchanged hunk again. It moves the old findings to the new revision.
- Result: low cost and low latency on large stacks that change frequently.

### D-03 Stack-aware AI review

- The reviewer gets the full stack as context, not only one PR.
- The reviewer can report a defect that crosses PR boundaries.
  Example: PR 2 removes a function that PR 4 calls.
- The reviewer can propose a split of a large PR into a stack.
  `grf split --suggest` shows the proposed cut points. The user accepts or rejects each cut.

### D-04 Proof-carrying findings

- Each AI finding must include evidence: the exact lines, the reason, and the expected failure.
- A second, independent verifier pass checks each finding. It rejects findings with weak evidence.
- Optional: the verifier writes a small failing test. CI runs the test. A failed test raises the confidence to "proven".
- The UI shows the confidence level on each finding: `proven`, `likely`, or `possible`.

### D-05 Noise budget

- Each repository has a noise budget. Default: 5 AI comments for each revision.
- Grafene ranks findings by severity multiplied by confidence. It posts only the top findings.
- The remaining findings go to a collapsed "more" section.
- Grafene records if a human accepts, resolves, or dismisses each finding.
- If the acceptance rate of a rule or prompt falls below a threshold, Grafene mutes it and tells the admin.

### D-06 Rules as code, and team memory

- Teams write rules in plain language in `.grafene/rules/*.toml`. The rules are in the repository, with version control.
- Teams can also write deterministic rules as WebAssembly (WASM) plugins.
- When reviewers repeat the same comment, Grafene proposes a new rule.
  Grafene opens a PR that adds the rule file. A human must approve it.
- Thus, team memory is visible, reviewable, and portable. It is not hidden in a vendor database.

### D-07 Impact map

- Grafene builds a symbol index with tree-sitter.
- For each PR, Grafene shows the changed symbols and their callers in other files.
- Grafene calculates a risk score from: blast radius, test coverage of changed lines, file churn, and author familiarity.
- Grafene suggests reviewers from code ownership and recent edits to the affected files.

### D-08 Guided review path

- Grafene does not sort files by name.
- Grafene sorts files in reading order: interfaces and types first, then logic, then tests, then generated files.
- The AI writes a short note for each step: "This file adds X. Look at Y."
- Grafene hides formatting-only and move-only hunks by default (see section 8.3).

### D-09 Intent check

- Grafene compares the PR description and the linked issue with the diff.
- Grafene reports scope drift. Example: "The description says 'fix typo'. The diff also changes auth logic."

### D-10 Terminal review client

- `grf tui` gives a full review client in the terminal (built with ratatui).
- The user can read diffs, add comments, approve, and merge without a browser.
- All actions have keyboard shortcuts. The web UI uses the same shortcuts.

### D-11 Cost and latency transparency

- Each AI review shows the model, the token count, the cost, and the duration.
- Admins set budget caps for each repository and each month.
- When a review exceeds its cap, Grafene uses a smaller model or skips low-risk hunks.

---

## 6. Architecture

### 6.1 Component overview

```
                 +-------------------+
   developer --> |  grf (CLI / TUI)  | --- git CLI + gix ---> local repo
                 +---------+---------+
                           | HTTPS (JSON) / local engine
                           v
+--------------------------------------------------------------+
|                    grafene-server (one binary)              |
|                                                              |
|  axum HTTP  --  web UI (SSR)  --  webhook intake             |
|      |                                  |                    |
|  domain services (stacks, PRs, reviews, queue, inbox)        |
|      |                                  |                    |
|  job runner (tokio, SQLite-backed)  --  review engine        |
|      |                                  |      |             |
|  SQLite / Postgres        tantivy index   rules + WASM host  |
+-----------+----------------------------------+---------------+
            |                                  |
            v                                  v
      forge APIs (GitHub, ...)          model providers
                                        (Claude API, local)
```

### 6.2 Technology choices

| Concern              | Choice                               | Reason                                                       |
|----------------------|--------------------------------------|--------------------------------------------------------------|
| Language             | Rust (stable, edition 2024)          | Speed, low memory, one binary.                               |
| Async runtime        | tokio                                | Standard. Good ecosystem.                                    |
| HTTP server          | axum + tower                         | Small, fast, composable.                                     |
| Database             | SQLite (WAL mode) via sqlx           | No extra service. Fast for one node.                         |
| Database (scale-out) | PostgreSQL via sqlx                  | Same queries, behind a feature flag.                         |
| Git read operations  | gix (gitoxide)                       | Pure Rust. Fast object and ref access.                       |
| Git write operations | `git` CLI (`rebase --update-refs`)   | Correct and complete rebase. gix rebase is not complete.     |
| Diff                 | imara-diff                           | Fast Myers and histogram diff.                               |
| Syntax and symbols   | tree-sitter                          | Highlight, semantic diff, and symbol index from one parser.  |
| Search               | tantivy                              | Embedded full-text search. No extra service.                 |
| Web UI               | askama templates + htmx              | Server-side render. Small JS payload. See 6.3.               |
| TUI                  | ratatui + crossterm                  | Fast, cross-platform terminal UI.                            |
| CLI parsing          | clap (derive)                        | Standard.                                                    |
| Plugins              | wasmtime                             | Sandboxed, portable rule plugins.                            |
| Forge client         | octocrab (GitHub) + own adapters     | Typed GitHub API.                                            |
| Serialization        | serde, serde_json, toml              | Standard.                                                    |
| Telemetry            | tracing + OpenTelemetry (optional)   | Structured logs and traces.                                  |
| Static assets        | rust-embed                           | Assets go inside the binary.                                 |

### 6.3 Web UI decision

Three options were compared.

| Option                     | JS/WASM size (gzip) | First render | Decision |
|----------------------------|---------------------|--------------|----------|
| askama SSR + htmx          | ~15 KB              | Fast         | Selected |
| Leptos (WASM, hydrate)     | 150–400 KB          | Medium       | Rejected for v1 |
| React SPA                  | 200 KB +            | Slow         | Rejected |

Reasons:

- The server renders diffs. The server already has the parsed tree-sitter data.
- htmx swaps HTML fragments. Most pages need no client state.
- A small vanilla JS module (< 20 KB) controls keyboard shortcuts and comment drafts.
- Large diffs use virtual scroll. The server sends hunks in pages of 200 lines.

### 6.4 Crate layout

Use one Cargo workspace.

```
grafene/
  crates/
    grf-core/      Domain types: Stack, Branch, Pr, Revision, Finding. No I/O.
    grf-git/       Git access. gix for reads. git CLI wrapper for rebase.
    grf-diff/      Text diff, interdiff, semantic diff, hunk hash.
    grf-index/     tree-sitter symbol index and call graph.
    grf-rules/     Rule parser, rule runner, WASM host.
    grf-ai/        Model providers, context builder, prompts, verifier.
    grf-engine/    Review pipeline. Uses diff, index, rules, ai. Used by CLI and server.
    grf-forge/     Forge trait. GitHub adapter. Webhook parse.
    grf-store/     Database schema, migrations, queries.
    grf-server/    axum app, web UI, job runner, merge queue.
    grf-cli/       The `grf` binary. Includes the TUI behind a feature flag.
  .grafene/       Example rules for the Grafene repository.
  docs/
```

Rules for the crate graph:

- `grf-core` has no dependencies on other Grafene crates.
- `grf-engine` must not depend on `grf-server`. The CLI must use it alone.
- Only `grf-store` contains SQL.

---

## 7. Stack model

### 7.1 Local metadata

- Grafene keeps stack metadata in Git, not in a local database.
- Each branch has a ref note at `refs/grafene/meta/<branch>`.
  The note contains: parent branch, parent commit at last restack, PR number.
- Thus, the metadata moves with `git push` and `git fetch` if the user wants it to.
- The CLI keeps an in-memory cache only. It reads refs on start. gix makes this fast.

### 7.2 Restack algorithm

1. Find the trunk and the ordered list of branches (topological sort on parent links).
2. For each branch from bottom to top, compare the recorded parent commit with the current parent tip.
3. If they are equal, skip the branch.
4. If they are different, run `git rebase --onto <new-parent> <old-parent-commit> <branch> --update-refs`.
5. If a conflict occurs, stop. Show the conflicting files. Wait for `grf continue` or `grf abort`.
6. Update the metadata refs after each successful step.

### 7.3 CLI commands (version 1)

| Command               | Action                                                      |
|-----------------------|-------------------------------------------------------------|
| `grf create <name>`   | Create a branch on the current branch. Commit staged changes. |
| `grf modify`          | Amend the current branch. Restack the branches above it.    |
| `grf submit [--stack]`| Push branches. Create or update PRs.                        |
| `grf sync`            | Fetch trunk. Delete merged branches. Restack.               |
| `grf up` / `grf down` | Move to the child or parent branch.                         |
| `grf log`             | Show the stack as a tree, with PR status.                   |
| `grf split [--suggest]`| Split the current branch. With `--suggest`, use AI cut points (D-03). |
| `grf fold`            | Merge the current branch into its parent.                   |
| `grf move --onto <b>` | Change the parent of the current branch.                    |
| `grf review`          | Run the review engine on the local stack (D-01).            |
| `grf tui`             | Open the terminal review client (D-10).                     |
| `grf continue` / `grf abort` | Continue or stop a restack after a conflict.         |

---

## 8. Review engine

### 8.1 Pipeline

The engine runs these stages for each new revision.
Each stage writes its output to the cache.

| Stage | Name            | Action                                                                 |
|-------|-----------------|------------------------------------------------------------------------|
| 1     | Diff            | Calculate the diff and the interdiff. Calculate hunk hashes.           |
| 2     | Cache lookup    | Find hunks with existing results (D-02). Remove them from the work set.|
| 3     | Deterministic   | Run TOML rules, WASM plugins, and imported SARIF from CI.              |
| 4     | Context         | Get changed symbols, callers, related tests, stack siblings, PR text.  |
| 5     | Triage          | A small model gives a risk class to each hunk: `skip`, `light`, `deep`.|
| 6     | Review          | A large model reviews `deep` hunks. A small model reviews `light` hunks.|
| 7     | Verify          | An independent pass checks each finding (D-04).                        |
| 8     | Rank and post   | Apply the noise budget (D-05). Post to the forge and the web UI.       |

### 8.2 Model providers

- `grf-ai` defines a `Provider` trait with two methods: `complete` and `stream`.
- Default provider: Anthropic Claude API.
  - Triage: `claude-haiku-5-5`.
  - Review: `claude-sonnet-5-5`.
  - Verify and stack-level review: `claude-opus-5-5`.
- Other providers: any OpenAI-compatible endpoint (local llama.cpp, vLLM, Ollama).
- The repository config can set a model for each stage.
- Use prompt caching for the stable prefix: system prompt, rules, and repository summary.

### 8.3 Semantic diff

- Parse the old and new file with tree-sitter.
- Classify each hunk:
  - `format` — only whitespace or layout changes.
  - `move` — the same subtree is in a different location.
  - `rename` — only an identifier changes, at all uses.
  - `logic` — all other changes.
- The UI hides `format` and `move` hunks by default. It shows `rename` as one summary line.
- The AI does not review `format` hunks.

### 8.4 Context budget

- The context builder has a token budget for each stage.
- Priority of context, from high to low:
  1. The hunk and 20 lines around it.
  2. The full changed function.
  3. Definitions of types and functions that the hunk uses.
  4. Callers of changed functions (from the symbol index).
  5. Related tests.
  6. Other PRs in the stack (summaries only).
- When the budget is full, the builder stops. It does not truncate an item in the middle.

### 8.5 Finding format

```json
{
  "id": "f_01J...",
  "rule": "ai.null-deref",
  "severity": "high",
  "confidence": "likely",
  "file": "src/auth/session.rs",
  "lines": [118, 124],
  "title": "Session can be None after expiry check",
  "evidence": "Line 118 returns early only for Expired. Line 124 unwraps the Option.",
  "suggestion": "Replace unwrap() with a match on None.",
  "hunk_hash": "b3:9f2c...",
  "model": "claude-sonnet-5-5",
  "cost_usd": 0.0041
}
```

---

## 9. Data model

SQLite schema (summary). All IDs are ULIDs. All times are UTC.

| Table          | Main columns                                                         |
|----------------|----------------------------------------------------------------------|
| `repo`         | id, forge, owner, name, default_branch, config_toml                  |
| `user`         | id, forge_login, email, settings                                     |
| `stack`        | id, repo_id, trunk, created_at                                       |
| `pr`           | id, repo_id, stack_id, number, parent_pr_id, position, state, author_id |
| `revision`     | id, pr_id, seq, head_sha, base_sha, pushed_at                        |
| `hunk`         | hash, file_path, kind (format/move/rename/logic)                     |
| `revision_hunk`| revision_id, hunk_hash, old_range, new_range                         |
| `finding`      | id, hunk_hash, rule_id, rule_version, model, severity, confidence, body |
| `finding_state`| finding_id, revision_id, state (open/accepted/dismissed/resolved), actor_id |
| `comment`      | id, revision_id, file_path, line, author_id, body, thread_id         |
| `review`       | id, revision_id, reviewer_id, verdict, created_at                    |
| `queue_entry`  | id, repo_id, stack_id, state, batch_id, enqueued_at                  |
| `job`          | id, kind, payload, state, attempts, run_after                        |
| `usage`        | id, repo_id, revision_id, model, tokens_in, tokens_out, cost_usd     |

Notes:

- `finding` links to `hunk_hash`, not to `revision`. This makes D-02 possible.
- `finding_state` holds the human feedback for D-05.
- The `job` table is the job queue. Workers use `UPDATE ... RETURNING` to claim jobs. No Redis is necessary.

---

## 10. Merge queue

- The queue accepts a full stack as one entry.
- The queue makes speculative batches: it tests entries 1..N together.
- If a batch fails, the queue bisects it to find the failed entry.
- The queue merges each PR of a stack in order. It retargets each child PR to trunk before the merge.
- The queue reads CI status from the forge. It does not run CI.

---

## 11. Performance budgets

These budgets are requirements. CI must measure them. A regression of more than 10% blocks the merge.

| Metric                                         | Budget         |
|------------------------------------------------|----------------|
| `grf` cold start (`grf log`, 20-branch stack)  | < 30 ms        |
| `grf sync` local work, 10-branch stack         | < 1 s (no network) |
| `grf review` deterministic stage, 1,000 lines  | < 200 ms       |
| Server binary size (release, stripped)         | < 40 MB        |
| Server idle memory (RSS)                       | < 50 MB        |
| Page time to first byte, p95                   | < 50 ms        |
| Diff render, 10,000 changed lines, first page  | < 100 ms       |
| Web JS payload (gzip)                          | < 40 KB        |
| Webhook to first AI comment, 300-line PR, p50  | < 60 s         |
| Restack review cost with no hunk changes       | 0 model tokens |

Methods to meet the budgets:

- Use `mimalloc` as the global allocator.
- Use `lto = "fat"`, `codegen-units = 1`, and `panic = "abort"` for release builds.
- Keep the CLI free of tokio when no network is necessary.
- Stream HTML responses. Send the page shell before the diff is complete.
- Cache parsed tree-sitter trees by blob ID.

---

## 12. Security and privacy

- Grafene stores no source code on disk, except the cache of parsed trees and hunks. Admins can turn off this cache.
- Grafene sends code to a model provider only if the repository config permits it.
- The repository config can set `ai.mode = "local"`. Then only local endpoints get code.
- Store forge tokens and API keys encrypted (AES-GCM). The key comes from an environment variable or a file.
- Verify all webhook signatures (HMAC-SHA256).
- WASM plugins run with no file system and no network access. Each call has a time limit and a memory limit.
- Use GitHub App installation tokens with minimum scopes.
- Log no source code and no prompts at the default log level.

---

## 13. Milestones

### 13.1 Rules for milestones

- Each milestone gives a result that a person can use or measure.
- Each milestone has exit criteria. A milestone is complete only when all exit criteria are true.
- The performance budgets in section 11 apply from the milestone that adds the measured component.
- Durations are estimates for one work stream. The plan uses two parallel work streams (A and B).
- "Depends on" gives the milestones that must be complete first. Milestones with no dependency between them can run in parallel.

### 13.2 Overview

| ID  | Name                                | Weeks | Depends on | Release gate   |
|-----|-------------------------------------|-------|------------|----------------|
| M0  | Workspace and CI                    | 2     | —          |                |
| M1  | Local stacks                        | 3     | M0         |                |
| M2  | GitHub submit and sync              | 3     | M1         | CLI alpha      |
| M3  | Server core                         | 3     | M0         |                |
| M4  | Web review UI                       | 4     | M2, M3     | Internal alpha |
| M5  | Semantic diff and index             | 3     | M0         |                |
| M6  | AI review pipeline                  | 4     | M4, M5     | Private beta   |
| M7  | Local-first review and TUI          | 3     | M6         |                |
| M8  | Stack-aware review and verifier     | 4     | M6         |                |
| M9  | Guided path and intent check        | 2     | M6         | Public beta    |
| M10 | Merge queue                         | 3     | M4         |                |
| M11 | Rules as code and team memory       | 3     | M6         |                |
| M12 | Impact map and reviewers            | 3     | M5, M6     |                |
| M13 | Insights, notifications, PostgreSQL | 3     | M4         |                |
| M14 | Release 1.0                         | 4     | M7–M13     | 1.0            |

Total effort: 47 stream-weeks. With two streams, the calendar time is approximately 29 weeks.

Critical path: M0 → M1 → M2 → M4 → M6 → M8 → M14.
The critical path is 24 weeks. Stream capacity adds approximately 5 weeks.

Schedule (week numbers are from the project start):

| Weeks | Stream A                    | Stream B                                      |
|-------|-----------------------------|-----------------------------------------------|
| 0–2   | M0 Workspace and CI         | M0 Workspace and CI                           |
| 2–5   | M1 Local stacks             | M3 Server core                                |
| 5–8   | M2 GitHub submit and sync   | M5 Semantic diff and index                    |
| 8–12  | M4 Web review UI            | Make the review evaluation set (for M6)       |
| 12–16 | M6 AI review pipeline       | M13 Insights, notifications, PostgreSQL (12–15) |
| 16–20 | M8 Stack-aware review       | M10 Merge queue (15–18), M11 start (18–20)    |
| 20–25 | M7 TUI (20–23), M9 (23–25)  | M11 end (20–21), M12 Impact map (21–24)       |
| 25–29 | M14 Release 1.0             | M14 Release 1.0                               |

### 13.3 Feature traceability

| Feature                             | Milestone                              |
|-------------------------------------|----------------------------------------|
| B-01 Stack CLI                      | M1                                     |
| B-02 Stack submit                   | M2                                     |
| B-03 Stack sync                     | M2                                     |
| B-04 PR inbox                       | M4                                     |
| B-05 Stack view                     | M4                                     |
| B-06 Diff review                    | M4                                     |
| B-07 AI review                      | M6                                     |
| B-08 Merge queue                    | M10                                    |
| B-09 Notifications                  | M4 (web), M13 (email, Slack, webhooks) |
| B-10 Insights                       | M13                                    |
| D-01 Local-first review             | M7                                     |
| D-02 Rebase-stable findings         | M6                                     |
| D-03 Stack-aware AI review          | M8                                     |
| D-04 Proof-carrying findings        | M8                                     |
| D-05 Noise budget                   | M6, M11 (auto-mute)                    |
| D-06 Rules as code, and team memory | M11                                    |
| D-07 Impact map                     | M12                                    |
| D-08 Guided review path             | M9                                     |
| D-09 Intent check                   | M9                                     |
| D-10 Terminal review client         | M7                                     |
| D-11 Cost and latency transparency  | M6                                     |

### 13.4 Milestone details

#### M0 — Workspace and CI (2 weeks)

Scope:

- Make the Cargo workspace with the crates in section 6.4. Empty crates are permitted.
- Write the domain types in `grf-core`.
- Set the release profile (section 11).
- Set up CI: `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` on Linux, macOS, and Windows.
- Make the benchmark harness. CI keeps the benchmark results for each commit on `main`.

Exit criteria:

- CI passes on the three operating systems.
- The benchmark job runs. It blocks a merge on a regression of more than 10%.
- `grf --version` completes in less than 10 ms.

#### M1 — Local stacks (3 weeks)

Scope:

- `grf-git`: read refs and objects with gix. Wrap `git rebase --update-refs`.
- Stack metadata refs (section 7.1).
- Restack algorithm (section 7.2).
- Commands: `create`, `modify`, `log`, `up`, `down`, `move`, `fold`, `continue`, `abort`.

Exit criteria:

- Tests restack a 20-branch stack correctly after a change at each position.
- Tests cover conflict, `continue`, and `abort`.
- A failed restack never loses a commit. A test makes sure of this.
- `grf log` on a 20-branch stack completes in less than 30 ms.

#### M2 — GitHub submit and sync (3 weeks)

Scope:

- `grf-forge`: forge trait and GitHub adapter.
- CLI login with the GitHub device flow. Keep the token in the OS keychain.
- `grf submit` and `grf sync`.
- Add a stack navigation table to each PR description.
- Use ETag conditional requests to decrease rate-limit use.

Exit criteria:

- `grf submit` on a 10-branch stack makes 10 PRs with the correct base branches.
- A second `grf submit` with no changes makes no write calls to GitHub.
- `grf sync` deletes merged branches and restacks the remainder.

Release gate: **CLI alpha.** The CLI is usable alone, without the server.

#### M3 — Server core (3 weeks)

Scope:

- `grf-store`: schema (section 9), migrations, SQLite in WAL mode.
- `grf-server`: axum app, GitHub App install, GitHub OAuth login, sessions.
- Webhook intake with HMAC-SHA256 verification.
- Job runner on the `job` table, with retries and backoff.
- Encrypted storage for secrets (section 12).

Exit criteria:

- The server writes a webhook event to the database in less than 20 ms (p95).
- The server rejects a webhook with an incorrect signature. A test makes sure of this.
- The job runner tries a failed job again, to a maximum of 5 attempts.
- Idle memory is less than 50 MB. The binary is less than 40 MB.

#### M4 — Web review UI (4 weeks)

Scope:

- `grf-diff`: text diff, interdiff between revisions, tree-sitter syntax colours.
- Pages: PR inbox (B-04), stack view (B-05), diff review (B-06).
- Inline comments, comment threads, suggested changes, approve, and request changes.
- Two-way sync of comments and reviews with GitHub.
- Keyboard shortcuts. Virtual scroll for large diffs.
- Web notifications (part of B-09).

Exit criteria:

- Page time to first byte is less than 50 ms (p95).
- The first page of a 10,000-line diff renders in less than 100 ms.
- The web JS payload is less than 40 KB (gzip).
- A comment made in Grafene shows on GitHub in less than 5 s. A comment made on GitHub shows in Grafene in less than 5 s.

Release gate: **Internal alpha.** The Grafene team reviews all Grafene PRs in Grafene.

#### M5 — Semantic diff and index (3 weeks)

Scope:

- Hunk hash (section 3).
- Hunk classification: `format`, `move`, `rename`, `logic` (section 8.3).
- `grf-index`: symbol index and call graph for Rust, TypeScript, Python, and Go.
- Cache of parsed trees by blob ID.

Exit criteria:

- On a test corpus, the classification is correct for at least 95% of hunks.
- A rebase with no content change gives the same hunk hashes.
- The index of a 100,000-line repository builds in less than 10 s. An update for one file takes less than 50 ms.

#### M6 — AI review pipeline (4 weeks)

Scope:

- `grf-ai`: `Provider` trait, Claude API provider, OpenAI-compatible provider, prompt caching.
- `grf-engine`: pipeline stages 1–8 (section 8.1). Stage 7 (verify) is a simple version in this milestone.
- `grf-rules`: TOML rules and SARIF import. WASM plugins come in M11.
- Rebase-stable findings (D-02). Noise budget and finding states (D-05). Cost display and budget caps (D-11).
- Review evaluation set: a corpus of at least 200 PRs with known defects and known clean changes. CI runs it for each change to prompts or models.

Exit criteria:

- A restack with no hunk changes uses 0 model tokens.
- No revision gets more AI comments than the noise budget.
- Webhook to first AI comment on a 300-line PR is less than 60 s (p50).
- Precision on the evaluation set is at least 60%. Record the recall as the baseline.

Release gate: **Private beta.** Not more than 10 external teams.

#### M7 — Local-first review and TUI (3 weeks)

Scope:

- `grf review` runs `grf-engine` on the local stack (D-01).
- `grf submit` sends local findings to the server. The server does not review those hunks again.
- `grf tui` (D-10): inbox, stack view, diff, comments, approve, and merge.

Exit criteria:

- `grf review` operates with no network access and a local model endpoint.
- After a local review, the server review uses 0 tokens for the reviewed hunks.
- The TUI starts in less than 50 ms. It can do all the actions that the M4 web UI can do.

#### M8 — Stack-aware review and verifier (4 weeks)

Scope:

- Stack context in the context builder (section 8.4, item 6).
- Cross-PR findings (D-03).
- `grf split --suggest` (D-03).
- Full verifier pass and confidence levels (D-04).
- Generated failing tests, behind a feature flag. The tests run in the user's CI.

Exit criteria:

- Precision on the evaluation set is at least 75%. Recall is not less than the M6 baseline.
- The engine finds at least 70% of the cross-PR defects in the evaluation set.
- Each posted finding shows a confidence level.

#### M9 — Guided path and intent check (2 weeks)

Scope:

- Reading-order file sort, with a note for each step (D-08).
- Hide `format` and `move` hunks by default (D-08).
- Intent check against the PR description and the linked issue (D-09).

Exit criteria:

- In a user test, reviewers prefer the guided order to alphabetical order on at least 70% of PRs.
- The intent check finds at least 80% of the scope-drift cases in the evaluation set.

Release gate: **Public beta.**

#### M10 — Merge queue (3 weeks)

Scope:

- Stack entries, speculative batches, and bisection on failure (section 10).
- Retarget child PRs before each merge.
- Queue page in the web UI. `grf merge` in the CLI.

Exit criteria:

- A test merges a 5-PR stack in the correct sequence with no manual steps.
- After a failed batch of N entries, the queue finds the failed entry in not more than log2(N) + 1 CI runs.

#### M11 — Rules as code and team memory (3 weeks)

Scope:

- Plain-language rules in `.grafene/rules/*.toml`.
- WASM plugin host with time and memory limits (section 12).
- Detection of repeated human comments. Automatic PRs that propose new rules (D-06).
- Auto-mute of rules with a low acceptance rate (D-05).

Exit criteria:

- A WASM plugin cannot read a file or open a network connection. A test makes sure of this.
- A plugin that runs longer than its time limit stops. The review continues.
- The engine proposes a rule after the same comment occurs 3 times.

#### M12 — Impact map and reviewers (3 weeks)

Scope:

- Impact map page: changed symbols and their callers (D-07).
- Risk score for each PR.
- Reviewer suggestions from code ownership and recent edits.

Exit criteria:

- The impact map for a 300-line PR renders in less than 200 ms.
- On historical data, at least one suggested reviewer is the actual reviewer for at least 60% of PRs.

#### M13 — Insights, notifications, PostgreSQL (3 weeks)

Scope:

- Insights pages (B-10).
- Email digest, Slack, and outgoing webhooks (B-09).
- PostgreSQL backend, behind a feature flag.

Exit criteria:

- All `grf-store` tests pass on SQLite and on PostgreSQL.
- Insights pages render in less than 100 ms for a repository with 50,000 PRs.

#### M14 — Release 1.0 (4 weeks)

Scope:

- GitLab adapter.
- User and admin documentation.
- External security audit. Repair all high and critical issues.
- Packages: container image, install script, Homebrew, Scoop, and `cargo install`.
- Publish the benchmark results for section 11.

Exit criteria:

- The release build meets all budgets in section 11.
- The security audit has no open high or critical issues.
- `grf submit` and `grf sync` pass the same tests on GitLab as on GitHub.

Release gate: **1.0.**

---

## 14. Risks

| Risk                                                       | Mitigation                                                   |
|------------------------------------------------------------|--------------------------------------------------------------|
| gix does not support a necessary write operation.          | Use the `git` CLI for that operation. Keep it behind `grf-git`. |
| AI findings have a high false-positive rate.               | Verifier pass (D-04). Noise budget and auto-mute (D-05).     |
| Model cost is too high on large monorepos.                 | Triage stage. Hunk cache (D-02). Budget caps (D-11).         |
| SQLite write contention at high webhook volume.            | One writer task with a channel. PostgreSQL for large installs. |
| tree-sitter grammars are missing for some languages.       | Fall back to text diff and line-based context. Show a notice. |
| GitHub API rate limits.                                    | Use webhooks, not polling. Use conditional requests (ETag).  |

---

## 15. Open questions

1. Must stack metadata refs be pushed to the remote by default, or kept local?
2. Must Grafene offer a hosted service, or only self-host in v1?
3. Must the verifier test generation (D-04) run in the user's CI, or in a Grafene sandbox?
4. Which licence: Apache-2.0/MIT dual, or a source-available licence for the server?
5. Must the TUI ship in the default `grf` binary, or as a separate binary to keep `grf` small?
