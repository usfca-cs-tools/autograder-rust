# Autograder (Rust)

This is a Rust port of the Python autograder. It supports local testing and class runs, Git clone/pull, GitHub Actions artifact ingestion, Canvas upload, date-based milestones, rollups, and viewing previously saved results.

## Build

- Requires Rust (stable).
- Build:
  - `cd autograder-rust`
  - `cargo build --release`

The binary will be at `target/release/grade-rs`.

## Configure

On first run, a default `config.toml` is created following this resolution order:
- `$GRADE_CONFIG_DIR/config.toml`, if set
- Traverse parent directories from the current dir until `$HOME`
- Fallback: `$HOME/.config/grade/config.toml`

The default file includes commented sections for `[Canvas]`, `[CanvasMapper]`, `[Config]`, `[Git]`, `[Github]`, and `[Test]`.

## Tests Repo Layout

- Tests are read from `<tests_path>/<project>/<project>.toml` (default `tests_path` is `~/tests`).
- `[[tests]]` entries support `$project`, `$project_tests`, `$digital`, `$name` substitutions.
- `[project]` supports `build` (`make` or `none`), `timeout`, `capture_stderr`, `subdir`, and `strip_output`.

## Usage

- Local repo test:
  - `grade-rs test -p project -n 01 [-v|--very-verbose] [--unified-diff] [--quiet] [--no-color]`

- Class run (local execution):
  - `grade-rs class -p project [-s alice bob] [-j N] [-v|--very-verbose] [--quiet] [--no-color]`
  - With dates: `grade-rs class -p project -d` (writes `project-<suffix>.json`)
  - Output streams incrementally as repos finish, but is printed in the original student order. Per‑test tokens are colored (green pass, red fail). A score histogram prints at the end in descending score order.

- Clone student repos:
  - `grade-rs clone -p project [-s alice bob]`
  - With a specific date: `grade-rs clone -p project --date "YYYY-MM-DD[ HH:MM:SS]"`
  - From dates.toml: `grade-rs clone -p project -d` (interactive selector)
  - Quiet by default; pass `-v/--verbose` to show full `git` output.

- Pull:
  - `grade-rs pull -p project [-s alice bob]`

- Exec (run a shell command in each repo):
  - `grade-rs exec -p project -e "git pull; make clean" [-s alice bob] [-j N] [-d]`
  - Uses `/bin/sh -c ...` in each repo directory; output streams in input order. Supports `-d` to apply a date suffix from `dates.toml`.

- GitHub Actions (download results):
  - `grade-rs class -p project -g [-s alice bob]`

- Canvas upload (from JSON):
  - `grade-rs upload -p project [--file project.json] [-d] [-v]`
  - With `-d`, shows an interactive list of `*.json` in the current directory (arrow keys). `-v` prints helpful progress (course/assignment IDs, mapping, skips).

- View (print saved results without executing):
  - `grade-rs view -p project [--file project.json] [-d] [--no-color]`
  - Reproduces the class output (one line per repo with colored tokens) from a selected JSON; prints the same histogram. With `-d`, selects a JSON via an interactive list.

- Rollup across dates:
  - `grade-rs rollup -p project -d`

## Notes

- Digital integration: `$digital` is substituted in test command lines. Point `[Test].digital_path` at your `Digital.jar`.
- Colors: class and view colorize per‑test tokens (green pass, red fail). Disable with `--no-color` or `NO_COLOR=1`.
- Diffs: `-v/--very-verbose` can show compact or unified diffs for test mismatches; use `--unified-diff` with `-v` to see full diffs.
- Repo suffixes: `-d/--by-date` appends `-<suffix>` (from `dates.toml`) to repo paths and JSON filenames. The date selector is interactive (arrow keys, Enter to select, q/Ctrl‑C to abort).
- Incremental output: class and exec print results as they complete; ordering remains the original student order. Use `-j N` to control parallelism. With `-j 1`, runs sequentially.

## Limitations

- Some UI behaviors rely on ANSI escape sequences (required for the interactive selectors and color). On non‑TTY or terminals without ANSI support, selection falls back or aborts, and color can be disabled.
- The `view`/`upload -d` JSON selector lists all `*.json` in the current directory; ensure you’re in the directory containing your class JSONs.

## Tips

- Quiet class output while still writing JSON: use `--quiet`.
- Troubleshooting Canvas: use `upload -v` to see mapping and skip details.
- Verifying a date suffix: histograms and JSON filenames include the selected suffix (e.g., `project-due.json`).
