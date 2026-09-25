# Dart Decimate Design

## Overview

Dart Decimate has no application UI. Its interfaces are a command-line report
for developers, machine-readable output for CI and agents, a self-contained HTML
report, and an MCP server. Every surface renders the same deterministic report
data; see [PRODUCT.md](PRODUCT.md) for the audience.

## Colors

The HTML report uses one dark theme defined as CSS custom properties in
[src/output/html/assets.rs](src/output/html/assets.rs): `--bg` `#090909`,
`--surface` `#121212`, `--surface-raised` `#181818`, `--line` `#2d2d2f`,
`--line-strong` `#3a3a3d`, `--text` `#f4f4f5`, `--muted` `#ababaf`,
`--accent` `#8fdcff`, `--focus` `#c7f0ff`, and severity colors `--error`
`#ff6b6b`, `--warn` `#ffd166`, `--pass` `#64d98a`. Human terminal output
uses ANSI colors and turns them off when `NO_COLOR` is set
([src/output/human.rs](src/output/human.rs)).

## Typography

The HTML report uses the system sans-serif stack at 15px/1.5 for text and a
system monospace stack for rules, locations, and code. Labels are small
uppercase text with letter spacing.

## Layout

The HTML report is a single column capped at 1120px with metric grids that wrap
by available width and a narrower layout below 720px. Motion is enabled only
under `prefers-reduced-motion: no-preference`.

## Components

This is a nonvisual CLI product, so visual Atomic Design does not apply beyond
the HTML report. The actual interfaces are:

- Human report ([src/output/human.rs](src/output/human.rs)): verdict, summary
  counts, grouped findings with paths and line numbers, and next steps.
- JSON, SARIF, and schemas ([src/output](src/output), [docs/issue-types.md](docs/issue-types.md)):
  stable, agent-readable findings and summaries.
- HTML report ([src/output/html.rs](src/output/html.rs)): hero summary, issue
  summary metrics, a search and type filter, collapsible finding groups with
  evidence, and next steps; decision surfaces render as a separate report.
- MCP server ([src/mcp](src/mcp)): tools that expose the same analysis to
  agents.

## Do's and Don'ts

- Do keep output deterministic and include paths and line numbers for every
  finding.
- Do keep interactive HTML controls at least 44px tall with a visible
  `--focus` outline on keyboard focus.
- Do respect `NO_COLOR` and `prefers-reduced-motion`.
- Don't report style preferences as findings or claim unsupported features work;
  return clear unsupported JSON instead.
