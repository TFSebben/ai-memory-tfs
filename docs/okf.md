# OKF conformance (2.0)

ai-memory's wiki is natively an **Open Knowledge Format** bundle from
2.0 on: every page a consumer reads off disk is a conformant OKF
concept file, and a project's wiki directory is a conformant bundle.
"Native" means the wiki files *are* the OKF files — no export step
forks the truth (an `export --okf` / `import --okf` pair still exists
for moving bundles across tools).

## Target: OKF v0.2

Spec: `GoogleCloudPlatform/knowledge-catalog`, `okf/SPEC.md` (verified
2026-09-01). v0.2 supersedes v0.1 with two breaking changes
(`timestamp` → `generated: {by, at}`; the `# Citations` body section →
`sources` frontmatter) and additive trust/lifecycle/provenance
families. Summary of what conformance requires:

- every non-reserved `.md` file: parseable YAML frontmatter with a
  non-empty **`type`**;
- bundle root `index.md` declares **`okf_version: "0.2"`** (the only
  index.md frontmatter allowed) and lists the directory;
- reserved names `index.md` / `log.md` follow spec structure when
  present;
- consumers MUST tolerate unknown keys — all ai-memory extension
  fields are spec-safe as-is.

## Field mapping

| OKF key | ai-memory source |
|---|---|
| `type` (required) | derived from path family + existing frontmatter: `sessions/` → `Session Summary`, `_rules/` → `Rule`, `gotchas/` → `Gotcha`, `decisions/` → `Decision`, `procedures/` → `Procedure`, `concepts/` → `Concept`, `notes/` → `Note`, `runbooks/` → `Runbook`, `_slots/` → `Invariant`/`State` (from `slot_kind`), `_lint/` → `Lint Report`, `_pending/` → `Pending Note`; `kind:` frontmatter (`fact`/`note`/`procedure`/`decision`) wins over the path default when present |
| `title` | already written by every producer |
| `description` | existing `summary` field, when present |
| `tags` | already written |
| `generated.by` | actor convention: `process:ai-memory/<version>` for the zero-LLM consolidator and system writers; `<provider-model>` (e.g. `openai-compat/qwen3:32b`) for LLM-written pages; `human:<user>` for wiki edits attributed via the watcher |
| `generated.at` | the page version's `updated_at` |
| `sources` | session provenance: pages already stamped with `session_id`/`agent` get `[{resource: "ai-memory://session/<uuid>", author: "<agent>"}]` |
| `stale_after` | existing `expires_at` (TTL), when present |
| `status` | `deprecated` when TTL-expired but retained; otherwise omitted (spec default `stable`) |

Extension fields kept verbatim (unknown keys are conformant): `tier`,
`kind`, `slot_kind`, `entities`, `pinned`, `consolidated`,
`session_id`, `agent`, `summary`, `expires_at`.

## Bundle boundary

One **project scope directory = one bundle**: the portable unit of
knowledge is a project. Each project dir gets a generated `index.md`
(frontmatter `okf_version: "0.2"`, body = directory listing). The
existing `_meta.md` scope manifest is unchanged — it is ai-memory's
identity record; `index.md` is the OKF-facing description. Nothing in
the current tree writes `index.md` or `log.md` (verified), so the
reserved names are free. `log.md` is not adopted: git is the log.

## Enforcement: one choke point

Every page write funnels through `ops::upsert_page_in_tx`. A
deterministic `okf::conform_frontmatter(path, frontmatter, meta)`
normalization runs there for every new version: fills `type` /
`generated` / `sources` / `stale_after` from the mapping above,
touches nothing already present, invents nothing non-derivable.
Determinism matters: the identical-content idempotency check hashes
frontmatter, so conforming the same input twice must yield identical
bytes.

## Migration of existing stores

Order is fixed; each step gates the next:

1. **Proactive backup, first, always.** The migration compresses the
   entire data dir (wiki, SQLite DB, manifests) to
   `~/ai-memory-backup-pre-2.0-<date>.tar.gz` — outside the data dir —
   verifies the archive is listable and size-sane, and **aborts if the
   backup cannot be written or verified**. The archive path is recorded
   in the wiki meta manifest.
2. **In-place frontmatter rewrite.** Same page id, same version row,
   body untouched, `updated_at` untouched: no version explosion, no
   embedding invalidation, no `updated_at` stampede. One git commit
   ("okf-migration") on the wiki, after a pre-migration checkpoint
   commit. Reindex afterwards.
3. **`wiki_format` generation marker** in the wiki meta manifest;
   `serve` refuses to open a newer-generation wiki with an older binary
   (mirror of the DB schema-ahead guard).
4. **Idempotent**: a re-run migrates zero pages.
5. **Homepage notice** until the archive is deleted: path, size, date,
   plus "everything looks right → delete the archive" and "something
   is missing → restore steps" (linking `MIGRATION-2.0.md`).

Rollback: restore the archive (blunt, no git knowledge needed), or the
pre-migration git checkpoint + `reindex` (surgical).

## Tests (each with a control that must fail on a broken build)

- Round-trip: page → OKF file on disk → parsed back identical.
- Conformance: every file in a migrated store has parseable
  frontmatter + non-empty `type`; bundle root carries `okf_version`.
- No-churn: page ids, version rows, and `updated_at` byte-identical
  across migration (control: a migration that supersedes pages fails).
- Idempotency: second run migrates zero pages.
- Backup gate: archive step broken → migration refuses to run.
- Homepage notice renders the recorded archive path and clears when
  the file is gone.
- Foreign OKF v0.2 bundle imports into a project; `export --okf`
  emits a bundle a strict reader accepts.
- Retrieval regression: LongMemEval baseline re-run; no material drop.
