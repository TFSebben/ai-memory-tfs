# Benchmarks

Published retrieval-quality numbers for ai-memory, with full provenance
(commit, dataset sha256, hardware, mode). Every number here was produced
by the in-repo harness — see `evals/README.md` for how to reproduce:

```bash
cargo build --release -p ai-memory-cli
cargo run --release -p ai-memory-eval -- retrieval --fetch
```

## Baselines

| date | dataset | mode | overall hit@5 | file |
|---|---|---|---|---|
| 2026-09-01 | LongMemEval-S (v1) | zero-llm (FTS only) | 0.617 | [longmemeval-s-2026-09-01.md](longmemeval-s-2026-09-01.md) |

## Reading the numbers

- **mode: zero-llm** is the deterministic floor: no consolidation LLM, no
  embedder, no reranker — pure FTS5 + entity/graph fusion over
  hook-captured observations. It is the configuration every install has
  with no API key at all. Embedding-assisted modes are expected to score
  higher and will be published alongside when the local-embedding work
  lands (roadmap item 5).
- **Comparability.** Published numbers from other systems on this dataset
  (agentmemory 0.967 R@5, doobidoo/mcp-memory-service 0.804 R@5) are
  embedding-based retrieval over raw chat logs. Our `hit@5` is the
  comparable statistic, but our pipeline additionally pays for
  production-shaped capture: excerpts are bounded at the 2 KB privacy
  boundary, so evidence deep inside one long turn is genuinely out of
  reach of the index. That cost is real and deliberate — the benchmark
  measures the shipped system, not an idealised retriever.
- **Regression gate.** Roadmap items 2-6 re-run this benchmark; a change
  that lowers a slice materially is a regression to fix, not a note to
  publish.
