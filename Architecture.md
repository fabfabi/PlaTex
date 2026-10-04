# Architecture

## Purpose

PlaTex is a local-first Rust application for managing cooking recipes and rendering them into standardized LaTeX documents. The central design goal is to keep recipe content separate from formatting so the same structured recipe can be presented in multiple styles.

## High-level architecture

The system is organized into a few clear layers, and the implementation order is intentionally staged:

Phase 1: Persistence and data model
- SQLite database structure for recipe storage
- tables for recipes, ingredients, steps, tags, and metadata
- versioning and timestamps for local edits
- JSON import/export support for simple sharing

Phase 2: Local editing UI
- recipe creation and modification screens
- import and export controls
- local save workflow
- search, filtering, and basic editing actions

Phase 3: LaTeX generation
- export layer that converts the domain model into LaTeX source
- formatting templates for standardized recipe documents
- export to .tex and optional PDF generation

Supporting layers
- Domain layer
  - Recipe
  - Ingredient
  - Step
  - Tag
  - Metadata such as servings, time, difficulty, and notes
  - Render templates or style descriptors

- Persistence layer
  - SQLite database for local storage
  - JSON import/export for recipe sharing
  - version metadata and update timestamps

- Export layer
  - converts the domain model into LaTeX source
  - applies standard formatting rules
  - supports multiple document styles or templates

- UI layer
  - local editor for recipe creation and changes
  - save, import, export, and preview actions

## Design principles

1. Content first, formatting second.
   Recipes are structured data, not presentation.

2. Keep the app local-first.
   The app is designed for personal use and local editing.

3. Separate responsibilities.
   Domain logic, persistence, export, and UI should not be mixed together.

4. Keep the codebase small and explicit.
   Prefer straightforward Rust structs and functions over complex abstractions.

5. Make sharing simple but controlled.
   JSON export/import is the recommended sharing mechanism between users.

## Data flow

The normal flow is:

1. The database schema is defined and populated for recipe storage.
2. The UI allows adding, editing, importing, and exporting recipe entries.
3. The user edits recipe data through the local interface.
4. The UI sends data to the domain model.
5. The domain model validates and stores the recipe.
6. Storage saves it to SQLite.
7. A template is selected for the output.
8. The export layer renders the recipe as LaTeX.
9. The generated .tex file can be compiled into PDF or included in a larger document.

## Implementation order

The project should be developed in this order:

1. Define the SQLite schema and data model
2. Implement database CRUD operations and import/export support
3. Build the local editor UI for adding and modifying entries
4. Connect UI actions to persistence
5. Add export commands and LaTeX rendering
6. Validate generated documents and refine templates

This keeps the first milestone stable and reduces complexity before the presentation layer is added.

## Recommended storage model

SQLite should be the local source of truth for the application.

Suggested tables:

- recipes
- ingredients
- steps
- tags
- recipe_versions
- sync_metadata

Each recipe should include:

- id
- title
- description
- servings
- prep time
- cook time
- difficulty
- created_at
- updated_at
- version

Tags (`tags` + `recipe_tags`): a recipe has 0..n tags. Tags are identified by a uuid; names need not be unique. Each tag has a `kind`:

- `chapter`: groups recipes into chapters; `sort_order` defines the chapter order
- `content`: describes content, e.g. breakfast or vegan

JSON export (version 2) contains a top-level `tags` list, and each recipe lists its tag uuids. Import matches tags by uuid with the same modes as recipes: "Only add new" skips existing uuids, "Overwrite changes" updates them, including the recipe's tag assignments.

## Recommended sharing model

The project should use a lightweight sharing strategy:

- local SQLite database for active use
- JSON export/import for recipe exchange
- optional Git or sync-folder workflow for versioned sharing
- conflict detection on duplicate edits

This keeps the project simple and avoids a server dependency while still supporting two-person collaboration.

## LaTeX export

Layout and content are split:

- `templates/*.sty` hold the whole layout (page format, fonts, colors, macros). They are embedded into the executable via `include_str!` and listed in `export::TEMPLATES`.
- "Export LaTeX..." opens a dialog to pick a built-in template or upload a `.sty`; it writes the `.tex` and the chosen template as `platex.sty` into the same folder.
- `src/template_check.rs` (std only) lists the required commands/environments and checks a template for them. It runs on uploaded templates, in `cargo test` for built-in templates, and in `build.rs` for every `templates/*.sty`, so a broken stored template fails `cargo build`. A test also ensures every macro the export emits is in the required list.
- `src/export.rs` writes only semantic macro calls from `RecipeDetail` (e.g. `\begin{Rezept}`, `\Zutat{2 EL}{Olivenöl}`, `\Schritt{...}`) using plain string building and `escape_latex`.
- Chapter tags become `\Kapitel` pages ordered by `sort_order`; recipes without a chapter follow at the end under `\Kapitel{Sonstige}`; each recipe shows the symbol of the chapter it is printed in. The export passes a chapter key (lowercase, umlauts transliterated, e.g. `Soße` → `sosse`, `Sonstige` → `sonstige`); the reference template loads it from `icon/symbol_<key>.png`.
- A different design means a different `.sty`; the Rust code stays unchanged.

PDF compilation is a later, optional step: call an installed `lualatex`/`latexmk`, or embed the Rust TeX engine [Tectonic](https://tectonic-typesetting.github.io) to compile directly to PDF without a local TeX installation. Tectonic only compiles; the `.tex` is still generated by `export.rs`.

## Rendering model

The rendering layer should operate only on the recipe data model. It should not mutate or embed business logic.

For example:

- a recipe can be rendered as a card
- a recipe can be rendered as a printable page
- a recipe can be included in a menu or cookbook

All of these are output variants built from the same recipe data.

## Module layout

A minimal project layout could look like this:

- src/
  - domain/
    - recipe.rs
    - ingredient.rs
    - step.rs
    - template.rs
  - storage/
    - sqlite.rs
    - import_export.rs
  - export/
    - latex.rs
    - templates/
  - ui/
    - app.rs
    - editor.rs
  - main.rs

This keeps modules narrow and easy to follow.

## Rules for future development

- do not mix persistence and UI code
- do not put LaTeX formatting logic inside the recipe model
- do not add a database server unless sharing requirements change significantly
- do not over-engineer generic abstractions early
- prefer small, testable functions and clear module boundaries

## Summary

PlaTex is intentionally a focused project: a simple recipe editor, a local database, and a clean LaTeX export path. The architectural goal is to keep recipe content stable and reusable while allowing multiple output formats without code duplication or presentation leakage.
