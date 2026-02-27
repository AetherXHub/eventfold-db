# Code Review: Ticket 4 -- Documentation: Filesystem Assumptions in docs/design.md

**Ticket:** 4 -- Documentation: Filesystem Assumptions in docs/design.md
**Impl Report:** prd/008-batch-atomicity-crash-consistency-reports/ticket-04-impl.md
**Date:** 2026-02-26 16:30
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | "Filesystem Assumptions" subsection exists under "On-Disk Format" | Met | Added as `### Filesystem Assumptions` at line 86, nested under `## On-Disk Format` (line 70), immediately before `## In-Memory Model` (line 100). Correct heading hierarchy. |
| 2 | States ext4 with `data=ordered` is tested and supported | Met | First paragraph: "The supported and tested configuration is **ext4 with `data=ordered` journaling mode**" with note about Linux distribution defaults. |
| 3 | Explains `data=ordered` guarantee making `File::sync_all()` sufficient | Met | Second paragraph ("`data=ordered` guarantee") explains data-before-metadata ordering and concludes "This property makes a single `File::sync_all()` call sufficient for data durability after a write." Also correctly explains the failure mode prevented (stale/zero-filled content with larger inode size). |
| 4 | Directory fsync required after new file creation | Met | Fourth paragraph ("Directory fsync on new file creation") explains name-to-inode link durability, the failure mode (file inaccessible on restart), and correctly notes "only required on initial file creation, not on every append." |
| 5 | `File::sync_all()` maps to `fsync(2)`, not `fdatasync(2)` | Met | Third paragraph ("`File::sync_all()` semantics") explicitly states the mapping to `fsync(2)`, identifies `fdatasync(2)` as Rust's `File::sync_data()`, and explains the difference (metadata flush). |
| 6 | NFS, CIFS, FUSE not supported | Met | Fifth paragraph ("Unsupported filesystems") explicitly names all three with concrete examples of failure modes (NFS client-side caching, FUSE not honoring fsync). |
| 7 | XFS, btrfs, ZFS, tmpfs may work but not validated | Met | Sixth paragraph ("Other Linux filesystems") lists all four with per-filesystem nuance (XFS rename/dsync, btrfs COW, tmpfs no durability). Correct advisory tone ("should verify its fsync behavior independently"). |
| 8 | Quality gates pass (no Rust code touched) | Met | Verified: `cargo build`, `cargo clippy`, `cargo fmt --check`, `cargo test` all pass clean. Only `docs/design.md` was modified by this ticket. |

## Issues Found

### Critical (must fix before merge)
- None

### Major (should fix, risk of downstream problems)
- None

### Minor (nice to fix, not blocking)
- None

## Suggestions (non-blocking)
- None. The documentation is technically precise, well-structured, and appropriately scoped. Each paragraph addresses a distinct concern with concrete failure modes rather than abstract warnings. The XFS/btrfs/ZFS/tmpfs paragraph provides useful differentiation without being encyclopedic.

## Scope Check
- Files within scope: YES -- only `docs/design.md` was modified by this ticket
- Scope creep detected: NO
- Unauthorized dependencies added: NO

## Risk Assessment
- Regression risk: LOW -- documentation-only change; no Rust source code modified
- Security concerns: NONE
- Performance concerns: NONE
