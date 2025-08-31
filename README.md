# Autograder (Rust) — MVP

This is a Rust port of the Python autograder. It currently supports local testing and class runs, Git clone/pull, GitHub Actions artifact ingestion, Canvas upload, and dates/rollup.

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
  - `grade-rs test -p project02 [-n 01] [-v|--very-verbose]`
- Class run (local):
  - `grade-rs class -p project02 [-s alice bob]`
  - With dates: `grade-rs class -p project02 -d` (writes `project02-<suffix>.json`)
- Clone student repos:
  - `grade-rs clone -p project02 [-s alice bob]`
  - With a date: `grade-rs clone -p project02 --date "YYYY-MM-DD[ HH:MM:SS]"`
  - From dates.toml: `grade-rs clone -p project02 -d`
- Pull:
  - `grade-rs pull -p project02 [-s alice bob]`
- GitHub Actions (download results):
  - `grade-rs class -p project02 -g [-s alice bob]`
- Canvas upload (from JSON):
  - `grade-rs upload -p project02 [--file project02.json]`
- Rollup across dates:
  - `grade-rs rollup -p project02 -d`

## Notes

- Digital integration: `$digital` is substituted in test command lines. Point `[Test].digital_path` at your `Digital.jar`.
- Colors and diffs: verbose modes print expected/actual headers and a simple diff header; full line-by-line diffs are not yet implemented.
- Repo suffixes: using `-d/--by-date` will append `-<suffix>` to the local repo dir name for `clone` and use that suffix for class JSON output.

## Limitations and Next Steps

- No parallel execution yet.
- Limited diff output in verbose mode.
- Additional error and edge-case handling will be expanded as needed.

