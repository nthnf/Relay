# AGENTS Mentor Guidance Design

## Goal

Strengthen `AGENTS.md` so future agents understand that their role is not only to execute tasks, but also to mentor the user during implementation work. The update must preserve the existing project memory and architecture contracts while adding clearer expectations for teaching, research, and recommendation quality.

## Scope

This design covers:

- preserving the current `AGENTS.md` project memory and architecture rules
- adding explicit mentoring behavior for future agents
- adding explicit research workflow guidance for library, framework, SDK, API, CLI, and cloud-service questions
- requiring concise pros/cons and a recommendation when meaningful tradeoffs exist

This design does not change platform architecture, service ownership, or transport rules.

## Current Problem

The current `AGENTS.md` is strong on architecture boundaries and working rules, but it does not state clearly enough that future agents should act as implementation mentors. It also does not explicitly encode the desired workflow for library and framework guidance:

- use authoritative docs first
- explain how a library fits this project
- compare options when needed
- recommend one option instead of stopping at neutral summaries

Without this guidance, future agents may stay too execution-focused, omit tradeoff analysis, or answer external-technology questions from memory instead of documentation.

## Design Principles

- Keep the current architecture memory intact.
- Make the behavioral upgrade additive, not disruptive.
- Keep the wording concise and practical so future agents will actually follow it.
- Treat mentoring as implementation-focused guidance, not abstract lecturing.
- Prefer authoritative external documentation before memory-based answers.

## Proposed Structure Change

Keep the current sections:

- `Project Memory`
- `Core Architecture Contracts`
- `Working Rules For Future Agents`

Add one new section after `Working Rules For Future Agents`:

- `Mentoring And Research Expectations`

This keeps the existing file recognizable while adding a focused behavior contract.

## Proposed Mentoring Behavior

The new section should instruct future agents to:

- act as a technical mentor as well as an implementer
- explain why a recommended approach fits the project, not only what to type
- stay concise, concrete, and implementation-oriented
- compare realistic options with concise pros/cons when tradeoffs matter
- give a recommendation instead of leaving the user with an unresolved option list
- warn when a proposed approach conflicts with documented architecture or ownership boundaries

The tone should be balanced:

- helpful, not passive
- explanatory, not overly verbose
- recommendation-driven, not vague

## Proposed Research Workflow

The new section should instruct future agents to use this workflow when the user asks how to implement something with an external technology:

1. use `find-docs` / Context7 first for libraries, frameworks, SDKs, APIs, CLI tools, and cloud services
2. use web search only if authoritative documentation is unavailable or insufficient
3. explain the relevant documented behavior in project context
4. summarize the main tradeoffs and recommend the simplest correct option

This workflow should explicitly discourage:

- answering version-sensitive library questions from memory when docs are available
- dumping snippets without explaining integration consequences
- presenting multiple options without a recommendation

## Project-Specific Framing

The new wording should remind future agents to tie mentoring back to this repository's contracts, especially:

- gRPC for synchronous authoritative flows
- RabbitMQ for durable async propagation
- service-local Postgres ownership
- Redis restrictions and approved exceptions
- `bootstrap` as the canonical UI-facing aggregate read service
- `chat` as the durable source of truth for messages

This keeps third-party guidance grounded in the actual project rather than generic best practices.

## Recommended Edit Shape

The implementation should make a small, localized edit to `AGENTS.md`:

- preserve the current bullets in `Project Memory`
- preserve the current bullets in `Core Architecture Contracts`
- keep `Working Rules For Future Agents`
- add one or two mentor-oriented bullets there if needed
- add a new `Mentoring And Research Expectations` section containing the stronger mentoring and research workflow guidance

## Success Criteria

The update is successful if a future agent reading `AGENTS.md` would clearly understand that it should:

- help the user understand implementation choices
- consult authoritative docs first for external technologies
- use web search only as a fallback
- present concise pros/cons when tradeoffs are real
- recommend an option instead of stopping at neutral summaries
- keep all guidance aligned to this repository's architecture rules

## Risks And Mitigation

### Risk: The file becomes too verbose

Mitigation:

- keep the new section short and concrete
- avoid repeating architecture bullets already present elsewhere

### Risk: Mentoring wording becomes too generic

Mitigation:

- tie research and explanation behavior directly to this repo's architecture contracts

### Risk: Agents over-explain every answer

Mitigation:

- explicitly prefer concise, recommendation-driven explanations
- only use pros/cons when a real tradeoff exists

## Implementation Note

The workspace is not a git repository, so the implementation should update `AGENTS.md` directly without commit-related steps.
