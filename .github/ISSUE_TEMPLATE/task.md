---
name: Task
about: One crate, one fix, small enough for a pull request
title: "[task] "
labels: ""
assignees: ""
---

## Crate

Id from `crates.map.json`. Find it with `pwsh -File tools/task.ps1 -Keyword <word>`.

## Symptom

What is wrong?

## Done when

What result finishes this task?

## Out of scope

What must this change leave alone?

## Denylist

Leave these alone unless this task names them: cookie `rustpress_session`, option keys `_rustpress_session_`, tables `rustpress_sessions` / `rustpress_cache` / `rustpress_migrations`, template key `rustpress_version`, env vars `RUSTPRESS_*`, WordPress `wp_*` tables, `themes/`, `static/`, `wp-content/`, and `LICENSE`.
