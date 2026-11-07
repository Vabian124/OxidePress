# Architecture Decision Records (ADR)

OxidePress の重要なアーキテクチャ決定を記録する。
Documents important architectural decisions in the OxidePress project. OxidePress is a GPL-2.0-only fork of RustPress by Ryuji Yasukochi.

## ADR一覧 / ADR List

| # | Title | Status | Date | Language |
|---|-------|--------|------|----------|
| [001](001-php-bridge-mode.md) | PHP Bridge Mode の採否 | 承認。ドロップイン経路の「PHP ランタイムは無い」は [002](002-php-worker.md) が上書き。FPM とプロキシの却下は維持 | 2026-03-07 | JP |
| [001 (EN)](001-php-bridge-mode.en.md) | PHP Bridge Mode and Plugin Compatibility Strategy | Accepted. The "no PHP runtime" conclusion is superseded for the drop-in path by [002](002-php-worker.en.md). The FPM and proxy rejections stand | 2026-03-07 | EN |
| [002](002-php-worker.md) | ドロップインプラグインのための PHP CLI ワーカー一つ | 承認 | 2026-11-07 | JP |
| [002 (EN)](002-php-worker.en.md) | One PHP CLI worker for drop-in plugins | Accepted | 2026-11-07 | EN |

## ADRとは / What is ADR

Architecture Decision Records (ADR) は、ソフトウェアアーキテクチャに関する重要な決定と、その決定に至った経緯・根拠を記録するドキュメントである。

Architecture Decision Records document important software architecture decisions, the context behind them, and the rationale for the chosen approach.

### フォーマット / Format

- **Status**: Proposed / Accepted / Rejected / Deprecated
- **Context**: Why this decision was needed
- **Options considered**: Alternatives that were evaluated
- **Decision**: The chosen approach
- **Rationale**: Why this decision was made
- **Impact**: How this decision affects the project
