---
name: review
description: Review changed code for software craftsmanship principles. Run after implementing a feature or making changes.
context: fork
agent: Explore
---

Review the recently changed files in this project for software craftsmanship quality. Focus on these principles:

## What to check

1. **Naming** — Do functions, variables, and types have clear, descriptive names? Could someone unfamiliar with the code understand what they do?

2. **Single responsibility** — Does each function do one thing? Does each module have a clear, focused purpose?

3. **Readability** — Is the code easy to follow? Are there any clever tricks that sacrifice clarity? Could any logic be simplified?

4. **Explicitness** — Are match arms explicit rather than relying on catch-all patterns? Are types used to make invalid states unrepresentable?

5. **Duplication** — Is there repeated logic that could be extracted? But also: is any abstraction premature?

6. **Consistency** — Does the new code follow the patterns established in the rest of the codebase? (enum cycling via next/prev, channel-based communication, state machine patterns)

## How to review

1. Run `git diff HEAD~1` to see recent changes (or `git diff main` if on a feature branch)
2. Read the changed files in full to understand context
3. For each finding, explain:
   - What the issue is
   - Why it matters (connect to a craftsmanship principle)
   - A concrete suggestion for improvement
4. Also call out what's done well — good naming, clean structure, smart design choices

## Output format

Group findings by file. Use this format:

**file.rs** — Brief summary of the changes

- [naming] `function_name` — suggestion and why
- [good] `other_function` — what's done well and why it works

Keep feedback constructive and educational. This is a learning project.
