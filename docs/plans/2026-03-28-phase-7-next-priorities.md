# Phase 7 Next Priorities Snapshot

> Historical snapshot from 2026-03-28. This is not the current roadmap.

## Status

The original Tasks 1-5 have landed or been superseded by later Phase 7 work.

Current sources of truth:

- [阶段总览](/roadmap/phase-progress)
- [开发计划](/roadmap/development-plan)
- [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)

## Original intent

This snapshot captured the first Phase 7 priority order:

- lock projected task-handle behavior
- make task-handle ownership projection-sensitive
- open conservative projection write / reinit paths
- land the first `for await` staticlib slice
- evaluate the first async `dylib` slice

## Current reading

Keep this file only when auditing why Phase 7 chose a conservative async/runtime order. Do not treat it as an active implementation plan.
