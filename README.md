# DeSlopify

**Codebase LLM-Friction Analyzer** — measure how hard it is for an LLM agent to work on your codebase.

No LLM calls. No cloud dependency. Single binary. Works on any codebase.

---

## What It Does

DeSlopify computes a **Slop Index (0-100)** that answers one question:

> How much of an LLM's context window gets wasted on navigation and understanding before it can do productive work?

A codebase scoring 80 means an LLM agent burns most of its context just figuring out where things are. A codebase scoring 15 means the agent can get straight to work.

It runs **pure static analysis** — tree-sitter AST parsing, file stats, config detection, import graph analysis — with zero LLM calls and zero tokens spent.

## Quick Install

```bash
# macOS (Apple Silicon)
curl -sSL https://github.com/cursedcoder/deslopify/releases/latest/download/deslopify-v0.3.0-aarch64-apple-darwin.tar.gz | tar xz
sudo mv deslopify /usr/local/bin/

# macOS (Intel)
curl -sSL https://github.com/cursedcoder/deslopify/releases/latest/download/deslopify-v0.3.0-x86_64-apple-darwin.tar.gz | tar xz
sudo mv deslopify /usr/local/bin/

# Linux (x86_64)
curl -sSL https://github.com/cursedcoder/deslopify/releases/latest/download/deslopify-v0.3.0-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv deslopify /usr/local/bin/

# Linux (aarch64)
curl -sSL https://github.com/cursedcoder/deslopify/releases/latest/download/deslopify-v0.3.0-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo mv deslopify /usr/local/bin/

# From source
cargo install --path .
```

Then run:

```bash
deslopify .
```

Output:

```
SLOP INDEX: 42/100 (MESSY)

DIMENSION BREAKDOWN
  Test safety net           [####-]  4/5  (12)  test ratio: 0.08, no test framework config
  Architecture clarity      [###--]  3/5  ( 9)  218 files, max depth 6, 15 files >500 lines
  Coupling / blast radius   [##---]  2/5  ( 6)  avg fan-out: 8.2, 3 circular deps
  Documentation             [###--]  3/5  ( 6)  README present, no architecture docs
  Style consistency         [##---]  2/5  ( 4)  linter configured, naming consistency: 74%
  Context pressure          [####-]  4/5  ( 4)  ~320k tokens, max function 890 lines
  ... 4 more (use --verbose to show all)

TOP FIXES (by estimated index reduction)
  1. [-8 pts] Split src/engine.rs (1,432 lines, 23 functions)
  2. [-6 pts] Add tests for src/api/ (0 test files for 12 source files)
  3. [-4 pts] Resolve 3 circular dependencies in src/core/
  4. [-3 pts] Add architecture documentation (15 top-level modules, no guide)
  5. [-2 pts] Configure a formatter (inconsistent style across 47 files)

  Estimated index after all fixes: 42 -> 19
```

## Installation

### Pre-built binaries

Grab the latest release from [GitHub Releases](https://github.com/cursedcoder/deslopify/releases).

### From source

```bash
git clone https://github.com/cursedcoder/deslopify.git
cd deslopify
cargo install --path .
```

## Usage

```bash
deslopify .                           # Scan current directory
deslopify src/ lib/                   # Scan specific directories
deslopify --verbose                   # Show all 10 dimensions
deslopify --context-budget            # Show token cost estimates
deslopify --format json               # JSON report
deslopify --format llm                # LLM-consumable fix instructions
deslopify --ci --max-score 50         # Exit 1 if score > 50 (CI gate)
deslopify --ignore "tests/*"          # Exclude patterns
deslopify --no-git                    # Disable git history analysis
deslopify --git-months 3              # Set git lookback window to 3 months
```

## Scoring Model

DeSlopify rates 10 dimensions of LLM friction, each scored 0-5 and weighted by impact:

| Dimension | Weight | What it measures |
|---|---|---|
| Setup reliability | 10 | Can an agent build/run this? (dependency files, lockfiles, Docker, build scripts) |
| Architecture clarity | 15 | Can an agent find what it needs? (directory depth, file sizes, module organization) |
| Coupling / blast radius | 15 | One change = how many files? (import fan-in/fan-out, circular deps) |
| Style consistency | 10 | Are patterns predictable? (formatter/linter configs, naming uniformity) |
| Test safety net | 15 | Can an agent verify changes? (test ratio, framework config, test proximity) |
| Runtime predictability | 10 | Hidden side effects? (global mutable state, I/O patterns) |
| Feedback loop speed | 5 | How fast can an agent iterate? (CI config, test runner, scripts) |
| Documentation | 10 | Does the agent need to guess? (README, architecture docs, doc comments) |
| Dependency boundaries | 5 | Clean separation? (vendor dirs, generated code markers, gitignore) |
| Context pressure | 5 | Raw token cost (file sizes, function lengths, nesting depth) |

**Formula:** `SlopIndex = sum(weight * rating / 5)`, capped at 100.

**Verdicts:** 0-20 CLEAN / 21-40 ACCEPTABLE / 41-60 MESSY / 61-80 SLOPPY / 81-100 DISASTER

## Output Formats

### Terminal (default)

Colored, human-readable report with dimension breakdown and top fixes.

### JSON (`--format json`)

Machine-readable report for CI/CD pipelines. Includes all dimensions, context budget, recommendations, language breakdown, and duplicate detection stats.

### LLM (`--format llm`)

Structured instructions that an LLM agent can directly consume to improve the codebase. Prioritized fixes with estimated index reduction after each step.

Example:

```
CODEBASE IMPROVEMENT INSTRUCTIONS
Current Slop Index: 62/100 (SLOPPY)

You are tasked with reducing the Slop Index of this codebase.
Execute these fixes in priority order.

PRIORITY 1: Split large modules (estimated index after: 54)
Target: src/engine.rs
This file has 1432 lines and 23 functions with high coupling.
Action: Break this file into smaller modules of 200-300 lines each.
...
```

## Context Budget

Use `--context-budget` to see estimated token costs for a typical LLM agent interaction:

```
CONTEXT BUDGET ESTIMATE
  File discovery:     ~   900 tokens
  Code reading:       ~  8.5k tokens
  Dependency tracing: ~   800 tokens
  Comprehension:      ~  3.2k tokens
  ──────────────────────────────────────────
  Navigation cost:    ~ 13.4k tokens  (7.6% of usable context)
  Remaining for work: ~162.6k tokens
```

## Language Support

DeSlopify uses [tree-sitter](https://tree-sitter.github.io/) for universal AST parsing. Deep analysis (function complexity, import graphs, naming) works for:

- Python
- JavaScript / JSX
- TypeScript / TSX
- Rust
- Go
- Java
- C / C++
- Ruby

All other languages still get Layer 1 analysis (file stats, config detection, pattern matching). Language detection covers 30+ languages by file extension.

## CI Integration

Add to your GitHub Actions workflow:

```yaml
- name: DeSlopify Check
  run: |
    cargo install --path .
    deslopify . --ci --max-score 60
```

Or use the JSON output for custom processing:

```yaml
- name: DeSlopify Report
  run: |
    deslopify . --format json > deslopify-report.json
```

## How It Works

```
Layer 1: Instant Signals (no parsing)
  File stats, language detection, config detection, grep heuristics

Layer 2: AST Analysis (tree-sitter)
  Function complexity, import graphs, naming analysis, nesting depth, duplication

Scoring Engine
  10 weighted dimensions -> Slop Index 0-100

Recommendation Engine
  Rule-based templates -> prioritized fix suggestions
```

DeSlopify respects `.gitignore` and skips common noise directories (`node_modules`, `target`, `vendor`, `__pycache__`, `.venv`, etc.).

## License

MIT
