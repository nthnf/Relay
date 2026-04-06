# AGENTS Mentor Guidance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update `AGENTS.md` so future agents preserve the current project memory while behaving more clearly as balanced implementation mentors who use authoritative docs first and explain recommendations with concise tradeoffs.

**Architecture:** This is a small additive documentation change to one existing file. The implementation preserves the current project memory, architecture contracts, and working rules, then adds a focused mentoring and research workflow section instead of rewriting the whole file.

**Tech Stack:** Markdown, AGENTS project memory conventions, Context7 / `find-docs` workflow guidance, web search fallback guidance

---

## File Structure

- Modify: `AGENTS.md`
  - Preserve existing `Project Memory`, `Core Architecture Contracts`, and `Working Rules For Future Agents` content.
  - Add a concise mentoring and research guidance section.

### Task 1: Update AGENTS Mentor Guidance

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Re-read the current `AGENTS.md` structure before editing**

Read and preserve these sections:

```md
## Project Memory
## Core Architecture Contracts
## Working Rules For Future Agents
```

Expected outcome:
- The edit remains additive.
- Existing architecture and collaboration rules are not removed.

- [ ] **Step 2: Add concise mentor-oriented guidance to the working rules if needed**

Update or extend `## Working Rules For Future Agents` so it still says agents should preserve boundaries and ask for clarification, while also making room for mentor-style help.

Required guidance to preserve or reinforce:

```md
- Preserve service ownership boundaries.
- Preserve shared documentation conventions across services and platform docs.
- Ask for clarification instead of assuming when uncertainty materially affects the contract.
- Prefer concise, concrete, implementation-oriented documentation.
- Avoid inventing unrelated systems or platform components.
```

Allowed addition:

```md
- Explain implementation choices clearly when the user is deciding between valid options.
```

- [ ] **Step 3: Add a new `Mentoring And Research Expectations` section**

Add a new section after `## Working Rules For Future Agents` with wording that covers all of the following:

```md
## Mentoring And Research Expectations
- Act as a technical mentor as well as an implementer.
- When the user asks how to implement something with a library, framework, SDK, API, CLI, or cloud service, consult authoritative documentation first.
- Use `find-docs` / Context7 first for supported technologies.
- If authoritative docs are unavailable or insufficient, use web search as a fallback.
- Explain the recommended approach in the context of this repository's architecture and constraints.
- When multiple valid approaches exist, give concise pros and cons and recommend the simplest correct option.
- Do not answer version-sensitive external-technology questions from memory when documentation is available.
- Do not dump snippets without explaining why the approach fits or conflicts with this project.
- Keep explanations concise, concrete, and implementation-oriented rather than academic.
```

The wording must preserve the user's desired tone:
- balanced mentorship
- concise tradeoff analysis
- recommendation-driven answers

- [ ] **Step 4: Tie the new section back to project-specific constraints**

Ensure the new section explicitly anchors guidance to this repository's architecture, including references such as:

```md
- gRPC for synchronous authoritative flows
- RabbitMQ for durable asynchronous propagation
- service-local Postgres ownership
- Redis restrictions and approved exceptions
- `bootstrap` as the canonical aggregate read service
- `chat` as the durable source of truth for messages
```

Expected outcome:
- Future agents do not give generic library advice detached from the current platform contracts.

- [ ] **Step 5: Verify the resulting section headings and key mentor terms**

Run: `rg -n "^## |find-docs|Context7|web search|pros and cons|technical mentor|authoritative documentation" AGENTS.md`

Expected:
- `AGENTS.md` still contains the original main sections.
- `AGENTS.md` now contains `## Mentoring And Research Expectations`.
- The file includes the new docs-first research workflow and mentor-oriented wording.

- [ ] **Step 6: Re-read the full file for brevity and consistency**

Check manually that:
- no existing architecture rule was removed
- the new section does not contradict current project memory
- the new section is short enough to remain practical
- the mentoring tone is balanced, not overly verbose

Expected outcome:
- `AGENTS.md` is stronger and clearer, but still compact.
