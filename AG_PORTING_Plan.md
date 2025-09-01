Autograder Python → Rust Porting Plan

Goals
- Parity: Re-implement the core grading workflow in Rust with compatible CLI semantics where practical.
- Safety and robustness: Strong typing, controlled process execution, clear error reporting.
- Incremental delivery: Ship value early (local testing) and iterate (Git/GitHub/Canvas integrations).
- Maintainability: Modular design mirroring Python package structure; unit and integration tests.

Non-Goals (Initial Phases)
- 1:1 replication of every Python behavior on day one.
- Full Canvas/GitHub API parity in the first MVP.
- Supporting rarely used config permutations until core is stable.

High-Level Architecture (Rust)
- Binary: `grade-rs` in `autograder-rust/` (Cargo crate).
- Modules:
  - `cli`: argument parsing (subcommands: test, class, clone, pull, exec, upload, rollup).
  - `config`: config discovery and loading; default generation if missing.
  - `testcases`: parse project test TOML; run test cases; compare outputs; scoring.
  - `cmd`: safe process execution with timeout and output limits; optional file output capture.
  - `util`: colors, path helpers, simple formatting, histogram, JSON serialization shapes.
  - `git` (later): clone, pull, date-limited checkout.
  - `github` (later): Actions artifact download and parsing.
  - `canvas` (later): REST API client and upload pipeline.
  - `dates` (later): `dates.toml` loading and selector for by-date workflows.

Python → Rust Module Mapping
- `actions/cmd.py` → `cmd.rs`
- `actions/util.py` → `util.rs`
- `actions/test.py` → `testcases.rs`
- `actions/config.py` → `config.rs` (+ `cli.rs`)
- `actions/git.py` → `git.rs` (phase 2)
- `actions/github.py`, `actions/server.py` → `github.rs` (phase 3), `http.rs` (shared HTTP utils)
- `actions/canvas.py` → `canvas.rs` (phase 4)
- `actions/dates.py` → `dates.rs` (phase 5)
- `actions/rollup.py` → `rollup.rs` (phase 5)
- `actions/upload.py` → implemented inside `canvas.rs` orchestration (phase 4)

Milestones and Deliverables
1) Phase 0: Repo audit and plan
- Understand Python CLI surface and config; capture scope and risks.
- Deliver: This plan (`AG_PORTING_Plan.md`).

2) Phase 1: MVP core (local testing)
- Implement `grade-rs test` and `grade-rs class` for local execution.
- Features:
  - Config discovery and default creation (`~/.config/grade/config.toml` fallback logic).
  - Load tests TOML for a project (`~/tests/<project>/<project>.toml`).
  - Build step (`make` or `none`), timeout and output limit, stderr capture toggle.
  - Placeholders for `$project`, `$project_tests`, `$digital`, `$name` interpolation.
  - Comparison (case-insensitive by default), diff printing on verbose, histogram, JSON file for class.
- Deliver: Compilable crate in `autograder-rust/`, README snippet inside `Cargo.toml` metadata, basic instructions in this plan.

3) Phase 2: Git integration
- Implement `clone` and `pull` using `ssh`/`https` selection; `by-date` checkout using `rev-list`.
- Deliver: Functional `grade-rs clone|pull` that mirrors Python behavior.

4) Phase 3: GitHub Actions artifact download
- Implement `--github-action` path for `class` by downloading the latest artifact, extracting `grade-results.json`, and summarizing.
- Deliver: `github.rs` with API client; env/config token, robust error handling.

5) Phase 4: Canvas upload
- Implement `upload` reading a previously generated class JSON file and PUT results to Canvas.
- Deliver: `canvas.rs` and `http.rs` with auth headers, pagination where needed.

6) Phase 5: Dates and rollup
- Implement `dates.toml` loader and interactive selector; `rollup` logic port; support `-d/--by-date` naming scheme.
- Deliver: `dates.rs`, `rollup.rs` with same score aggregation semantics.

7) Phase 6: Digital integration
- Support `$digital` interpolation and execution sequences for Digital CLI tests.
- Deliver: Verified execution against a sample `.dig` test file.

8) Phase 7: UX polish, parity, and performance
- Match Python CLI flags; ensure help text; improve colors; better logging.
- Add parallelization for repos and/or testcases where safe.

9) Phase 8: Decommissioning and migration
- Validate parity on a representative course; document migration and fallbacks to Python.
- Deliver: Checklist and migration guide.

Config Discovery (Parity Plan)
- If `GRADE_CONFIG_DIR` set → `GRADE_CONFIG_DIR/config.toml`.
- Else walk upward from CWD until `config.toml` or reach `$HOME`.
- Else default to `$HOME/.config/grade/config.toml` (create with commented defaults on first run).

CLI Parity Targets
- Subcommands: `test`, `class`, `clone`, `pull`, `exec`, `rollup`, `upload`.
- Flags:
  - `-p/--project`, `-n/--test-name`, `-v/--verbose`, `-vv/--very-verbose`.
  - `-s/--students ...` (list), `-g/--github-action`, `-d/--by-date`, `-e/--exec`.

Testing Strategy
- Unit tests for: config resolution, TOML parsing, path interpolation, line normalization.
- Integration tests: run small dummy project (`make` target or `none`) and verify pass/fail scoring.
- Golden tests: compare JSON outputs for deterministic fixtures.

Risks and Mitigations
- Process execution differences (Windows/POSIX): centralize process logic; skip POSIX-only controls on Windows.
- I/O and encoding differences: treat output as UTF-8, handle decode errors with friendly messages.
- API rate limits (GitHub/Canvas): backoff and error summaries; local fallback where possible.

Success Criteria per Phase
- Phase 1: Runs `grade-rs test` and `grade-rs class` for local repos, producing correct scores and JSON summary, with parity on case sensitivity and diff options.
- Phase 2–5: Each feature validated against a subset of existing Python tests and real-world runs.

Implementation Notes (MVP in this repo)
- Crate name: `autograder_rust`; bin: `grade-rs`.
- Dependencies: `clap`, `serde`, `serde_json`, `toml`, `thiserror` (errors), optionally `reqwest` (later phases).
- Output colors via ANSI escapes (no extra deps) for portability.

Rollout Plan
- Keep Python tool as fallback until Phase 5 parity.
- Encourage early adopters for `test/class` locally; gather feedback.

