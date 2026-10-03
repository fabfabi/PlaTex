# Project rules

## Communication style

- Keep all answers as short and concise as possible.
- Provide details only when explicitly asked.
- Prefer direct, minimal responses over long explanations.

## Git and branch safety

- Never commit directly to `main` or `dev`.
- Always create a new feature branch before making commits.
- If the current branch is `main` or `dev`, stop and switch to a feature branch before continuing.
- Do not merge or push directly into protected branches.
- Use pull requests for all changes.
- Never bypass branch protection or force-push to `main` or `dev`.

## Scope and change discipline

- Stay within the current project scope.
- Do not add unrelated features or "nice-to-haves" unless explicitly requested.
- Keep changes small, focused, and testable.
- Keep edits focused and avoid unrelated file churn.
- Do not add broad scope or unrelated features.

## Architecture and code quality

- Prefer the smallest architecture that solves the problem.
- Add a new module or abstraction only when it clearly reduces complexity.
- Prefer simple Rust code over unnecessary abstractions.
- Separate domain logic, persistence, UI, and LaTeX export.
- Keep recipe content separate from LaTeX formatting.
- Persistence and UI code must not directly generate LaTeX output.
- Keep the project minimal, explicit, and maintainable.

## Validation and testing

- Run the smallest relevant validation command for each change.
- Do not claim work is complete without validation evidence.
- For business logic changes, add or update tests before or alongside the fix.
- Write unit tests for data validation, SQL mapping, and export logic.
- Prefer real behavior tests over mock-heavy tests.
- Avoid mock-heavy tests when real behavior can be tested directly.
- Keep tests small, clear, and focused on one behavior.
- Use unit tests to cover CRUD logic, schema assumptions, and LaTeX rendering rules.
- Update documentation when behavior or structure changes.
