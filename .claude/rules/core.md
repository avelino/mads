---
paths:
  - "crates/mads-core/**"
---

# mads-core rules

- No network, no process spawning, no reading of the environment. Anything that needs them is a trait here and an implementation in `mads-providers`.
- Errors are values: `Issue` for validation, `ToolOutput::fail` for tool calls, `thiserror` enums for library errors. No `panic`, no `unwrap` outside tests.
- A new validation rule needs: a code (`E##` error or `W##` warning) in `google/rules.rs`, a table-driven test with a passing and a failing case, and a row in `docs/reference/validation-rules.md`. Errors block export, warnings go to the report.
- A new or changed tool needs: typed args with `deny_unknown_fields` and `JsonSchema`, a test for every error code it can return, a check that a failed call does not mutate the workspace, and an entry in `docs/reference/mcp-tools.md`.
- Changing the export format means changing `data/` expectations: the golden test in `tests/golden.rs` is the contract. Do not edit `data/` to make a test pass.
- Prompts in `prompts/*.md` are product behavior. Change one only with a reason from a real run, and keep the `mission.rs` playbook assertions passing.
- Keep functions under 50 lines and nesting under 4. Prefer a small function over a comment.
