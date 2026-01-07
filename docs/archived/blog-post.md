# Beyond Vibe Coding: A Framework for AI-Assisted Development That Actually Works

*How I built a multi-cursor text editor through 116 conversations with AI agents—and what I learned about making AI
collaboration sustainable.*

---

I built a text editor in Rust. Multi-cursor editing, tree-sitter syntax highlighting, split views, CSV spreadsheet mode,
configurable keybindings—around 15,000 lines of code across 333 commits. The vast majority of it was written through
conversations with AI agents.

This isn't another "look what I built in a weekend" post. Token took months. And somewhere around week three, I realized
the interesting part wasn't the editor itself—it was figuring out how to sustain AI collaboration on a project that
doesn't fit in a single context window.

Most AI coding content falls into two camps: breathless hype ("I built a SaaS in 2 hours!") or dismissive skepticism ("
AI can only do toy projects"). Neither is useful. The reality is messier and more interesting: AI-assisted development
on complex projects is genuinely powerful, but it requires structure that nobody talks about.

Here's what actually worked.

---

## Why Most AI Coding Projects Stall

Before the framework, the failure modes. I hit all of them before finding patterns that worked.

**Context collapse.** You have a great session, make real progress, then come back the next day and the AI has forgotten
everything. You spend 20 minutes re-explaining the architecture. Multiply this by 50 sessions and you've lost days to
re-onboarding an amnesiac collaborator.

**Scope creep.** You ask the AI to fix a bug and it suggests refactoring three files, adding a new abstraction, and "
while we're here" improving the error handling. Each suggestion is reasonable in isolation. Together, they derail your
afternoon.

**Architectural drift.** Session 12 makes a locally-sensible decision that contradicts what you decided in Session 4.
Without explicit documentation, there's no source of truth. The codebase accumulates inconsistencies that compound.

**The 80% trap.** AI is remarkably good at getting features 80% working. The last 20%—edge cases, integration, polish—is
where projects die. The AI pattern-matches to common solutions; your specific edge cases aren't in the training data.

The insight that changed everything: **the problem isn't the AI's capabilities. It's that we treat AI like a junior
developer who remembers everything, when it's actually a brilliant expert with amnesia who needs explicit structure.**

Once I started treating AI collaboration as a documentation problem, everything clicked.

---

## The Framework: Three Modes of Work

At the start of every session, I explicitly state which mode I'm in:

| Mode        | Purpose                            | Inputs                       | Example                             |
|-------------|------------------------------------|------------------------------|-------------------------------------|
| **Build**   | New behavior that didn't exist     | Feature spec, reference docs | "Implement split view rendering"    |
| **Improve** | Better architecture, same behavior | Organization docs, roadmap   | "Extract modules from main.rs"      |
| **Sweep**   | Fix a cluster of related bugs      | Gap document, bug tracker    | "Multi-cursor selection edge cases" |

This sounds almost stupidly simple. It's not. The mode declaration does two things:

First, it prevents scope creep. When I say "We're in BUILD mode implementing the CSV viewer," the AI doesn't suggest
refactoring the rendering pipeline. If it does, I point back to the mode. Clear boundaries.

Second, it sets expectations for what "done" looks like. Build mode ends when the feature works. Improve mode ends when
tests still pass and the code is cleaner. Sweep mode ends when the bug list is empty. Without explicit modes, sessions
drift toward an undefined "better" that's never finished.

Here's what a session start actually looks like:

```
Mode: BUILD
Feature: CSV viewer with spreadsheet UI
Reference: docs/feature/CSV_VIEWER.md
Scope: Phase 1 only—grid rendering and navigation. No cell editing yet.

Do not suggest improvements to existing rendering code.
Focus only on the new CsvViewer module.
```

Three sentences of context. Massive reduction in drift.

---

## Design Before Code

Here's the counterintuitive insight: with AI collaboration, documentation *before* implementation becomes more valuable,
not less.

When you're coding alone, you can hold the architecture in your head. The tradeoffs, the reasons for decisions, the edge
cases you considered—they're all accessible through memory. Documentation is a favor to your future self.

With AI, documentation is load-bearing infrastructure. The AI can't remember Session 4. It can't infer why you chose
approach A over approach B. It will confidently make decisions that contradict your existing architecture because it
doesn't know that architecture exists.

I use three types of documents:

### Reference Documentation

A "document of truth" for cross-cutting concerns. For Token, this
is [EDITOR_UI_REFERENCE.md](https://github.com/HelgeSverre/token/blob/main/docs/EDITOR_UI_REFERENCE.md)—a 2,000-word
spec defining viewport math, coordinate systems, cursor behavior, and scrolling semantics.

This document gets referenced in almost every session. "See EDITOR_UI_REFERENCE.md for how we calculate visible lines."
The AI reads it, internalizes the definitions, and makes consistent decisions.

Before I had this document, every session reinvented viewport calculations slightly differently. After, consistency.

The real power came from having the AI review the document before I wrote any code. I asked Claude (in "Oracle" mode) to
analyze EDITOR_UI_REFERENCE.md for logical errors, edge cases, and unstated assumptions. It found 15 issues:

- Off-by-one error in `lastVisibleLine` calculation
- Division-by-zero edge case in scrollbar thumb sizing
- Inconsistency between documented "anchor/head" selection model and actual "start/end" semantics
- Missing coverage for IME composition, BiDi text, soft-wrap interaction

Fifteen potential bugs caught before a single line of implementation. The review took 20 minutes. The bugs would have
taken hours to find through testing.

### Feature Specifications

Written before implementation, not after. For the multi-cursor feature, I
wrote [SELECTION_MULTICURSOR.md](https://github.com/HelgeSverre/token/blob/main/docs/archived/SELECTION_MULTICURSOR.md)
containing:

- Data structures and their invariants
- Keyboard shortcuts table
- Message enums and expected behavior for each
- A phased implementation plan

The phased plan is critical. Instead of "implement multi-cursor editing," which is a week of work that's hard to
checkpoint, I had:

- Phase 0: Per-cursor primitive operations
- Phase 1: All-cursor wrapper functions
- Phase 2: Update keyboard handlers
- Phase 3: Update mouse handlers
- Phase 4: Add tests
- Phase 5: Bug sweep

Each phase is a session or two. Progress is visible. The AI can focus on one phase without being overwhelmed by the
whole feature.

### Gap Documents

For features that are 60-90% complete, a gap document converts "vague incompleteness" into concrete tasks.

[MULTI_CURSOR_SELECTION_GAPS.md](https://github.com/HelgeSverre/token/blob/main/docs/archived/MULTI_CURSOR_SELECTION_GAPS.md)
listed:

- What's implemented vs. what's missing
- Design decisions needed for each gap
- Test cases and success criteria

This is where the Sweep mode shines. "Here are 7 gaps. Each has acceptance criteria. Work through them systematically."
The AI is excellent at applying patterns across multiple instances once you've defined the pattern clearly.

---

## Case Study: The Multi-Cursor Migration

Adding multi-cursor support to a single-cursor editor is a textbook "touches everything" change. Cursor movement,
selection handling, text insertion, deletion, undo/redo—every operation that assumed one cursor now needs to handle N
cursors.

Here's how the framework played out:

**Step 1: Write invariants upfront.**

Before any code, I documented the core invariants:

```rust
// MUST maintain: cursors.len() == selections.len()
// MUST maintain: cursors[i].to_position() == selections[i].head
// MUST maintain: cursors sorted by position, no overlaps after merge
```

These three lines prevented a category of bugs. Every session, the AI knew what properties to preserve. When I got a
failing test, I could often diagnose it by asking "which invariant is violated?"

**Step 2: Create migration helpers.**

The old code assumed `self.cursor` existed as a single field. Changing every reference at once would be a massive
breaking change. Instead:

```rust
impl EditorState {
    // Old code still works via accessor
    pub fn cursor(&self) -> &Cursor {
        &self.cursors[0]
    }

    // New code uses explicit indexing
    pub fn cursor_at(&self, idx: usize) -> &Cursor {
        &self.cursors[idx]
    }
}
```

This pattern let me migrate incrementally. Old code kept working. New code could use the explicit API. No big-bang
rewrite.

**Step 3: Implement in phases.**

Following the spec:

- Phase 0: Implement `move_cursor_left_at(idx)`, `move_cursor_right_at(idx)`, etc.—operations on a single cursor by
  index
- Phase 1: Implement `move_all_cursors_left()` that iterates all cursors and calls the Phase 0 primitives
- Phase 2: Update keyboard handlers to use the all-cursors versions
- Phase 3: Update mouse handlers (Option+Click to add cursor)
- Phase 4: Add test coverage
- Phase 5: Bug sweep

Each phase was 1-3 AI sessions. I could pause after any phase with working code.

**Step 4: Run targeted sweeps.**

After Phase 4, I had working multi-cursor editing with some edge case bugs. Instead of fixing them one-off as I found
them, I created a gap document listing all known issues, then ran a dedicated Sweep session.

Why does this matter? The AI is excellent at "apply this fix pattern to similar cases." When I said "here are 7
selection merge bugs, all caused by not calling `merge_overlapping_selections()` after the operation," the AI fixed all
7 consistently. One-off fixes would have been slower and less consistent.

The multi-cursor implementation threads are
public: [T-d4c75d42](https://ampcode.com/threads/T-d4c75d42-c0c1-4746-a609-593bff88db6d), [T-6c1b5841](https://ampcode.com/threads/T-6c1b5841-b5f3-4936-b875-338fd101a179), [T-e751be48](https://ampcode.com/threads/T-e751be48-ab56-4b90-a196-d5df892d955b).
You can see exactly how the conversations played out.

---

## Agent Configuration: AGENTS.md

Every project needs an AGENTS.md (or CLAUDE.md, or whatever your tool uses) file. This is the context the AI can't
infer.

Mine includes:

```markdown
## Build Commands

- `make build` - Debug build
- `make test` - Run all tests (uses cargo nextest)
- `make dev` - Build and run with sample file

## Architecture

- Elm Architecture: Model → Update → View
- All state in AppModel
- Messages are enums, update() pattern-matches

## Conventions

- Module tests go in `tests/` directory, not inline
- Use `saturating_add`/`saturating_sub` for cursor math
- New features need a doc in `docs/feature/` first

## Key Files

- `src/model/` - Core data structures
- `src/update/` - Message handling (5 submodules)
- `docs/EDITOR_UI_REFERENCE.md` - Viewport/coordinate reference
```

This takes 30 minutes to write. It saves hours of correcting wrong assumptions.

Without AGENTS.md, the AI guesses your conventions. It will run `cargo test --all-features` instead of `make test`. It
will put tests inline instead of in your tests directory. It will use wrapping arithmetic instead of saturating. Each
correction is minor; the cumulative friction is significant.

---

## The Research → Synthesize → Implement Pattern

For genuinely novel features—things I hadn't built before—I added a research phase before implementation.

The keymapping system is a good example. I needed configurable keyboard shortcuts with context-aware bindings (different
behavior when a modal is open vs. in the editor). I'd never built this.

**Step 1: Research.**

I used the AI as a research assistant: "How do VSCode, Helix, Zed, and Neovim implement configurable keymaps? Focus on
data structures and precedence rules."

The output: a comparison of four different approaches.

| Editor | Pattern                        | Key Insight                                    |
|--------|--------------------------------|------------------------------------------------|
| VSCode | Flat vector + context matching | User overrides via insertion order             |
| Helix  | Trie-based                     | Efficient prefix matching for modal editing    |
| Zed    | Flat with depth indexing       | Context depth + insertion order for precedence |
| Neovim | Lua scripting                  | Full programmability                           |

**Step 2: Synthesize.**

Based on the research, I wrote a design doc choosing the flat-vector approach (simpler than trie, sufficient for my
needs) with YAML config (easier to hand-edit than JSON or TOML for this use case).

The design doc included the config format, the data structures, the matching algorithm, and a phased implementation
plan.

**Step 3: Implement.**

Only now did I start coding. The implementation sessions referenced the design doc constantly. No architectural
decisions during implementation—those were already made.

The keymapping research thread: [T-35b11d40](https://ampcode.com/threads/T-35b11d40-96b0-4177-9c75-4c723dfd8f80). The
implementation resulted in 74 default keybindings with platform-aware modifiers (Cmd on macOS, Ctrl elsewhere) and
context conditions.

---

## Module Extraction: An Improve Mode Example

Sometimes the codebase needs restructuring without new features. This is Improve mode.

By early December, main.rs had grown to 3,100 lines. It worked, but navigation was painful and the file was doing too
many things.

I ran a series of extraction sessions:

1. Extract `update_layout` and helpers → `update/layout.rs`
2. Extract `update_document` and undo/redo → `update/document.rs`
3. Extract `update_editor` → `update/editor.rs`
4. Extract `Renderer` and rendering code → `view.rs`
5. Extract `PerfStats` and overlay → `perf.rs`
6. Extract `handle_key` → `input.rs`
7. Extract `App` and `ApplicationHandler` → `app.rs`

After: main.rs was 20 lines. Seven focused modules. All 669 tests passing.

The key to Improve mode: tests are your invariant. "Refactor this, all tests must still pass" is a clear success
criterion. The AI can be aggressive about restructuring because the tests catch regressions.

Threads for this extraction sprint: [T-ce688bab](https://ampcode.com/threads/T-ce688bab-2373-4b8e-bf65-436948e19853)
through [T-072af2cb](https://ampcode.com/threads/T-072af2cb-28ed-4086-8bc2-f3b5c5a74ab7).

---

## What I'd Do Differently

Authenticity requires acknowledging what didn't work.

**I should have written EDITOR_UI_REFERENCE.md earlier.** The first few weeks had inconsistent viewport calculations
because I was figuring it out session-by-session. Writing the reference doc forced clarity. I should have done it in
week one, not week three.

**Mode discipline slipped sometimes.** When I forgot to state the mode explicitly, sessions drifted. I'd ask for a bug
fix and get a refactoring suggestion and think "that's a good idea" and suddenly I'm doing two things at once and
finishing neither. The modes work, but only if you actually use them.

**Gap documents work best when written before frustration.** I wrote MULTI_CURSOR_SELECTION_GAPS.md after three
frustrating sessions of whack-a-mole bug fixing. If I'd written it after Phase 4 (when I knew the feature was mostly
working but had rough edges), I would have saved those three sessions.

**Some research was overkill.** Not every feature needs a four-editor comparison study. For simpler features, a quick "
how do other editors handle X?" question in the same session is enough. The research phase is for genuinely novel
problems, not routine features.

---

## The 116 Threads

Everything I've described is documented in 116 public conversation threads
on [my Amp Code profile](https://ampcode.com/@helgesverre).

This matters because most AI development content is "trust me, this worked." You can't verify it. You can't see the
actual prompts, the false starts, the corrections.

With Token, you can. The research threads, the bug hunts, the refactoring sessions—they're all there. When I say "the
Oracle review found 15 issues in my reference doc," you can read that thread. When I say "the multi-cursor
implementation took 5 phases," you can see each phase.

This transparency isn't altruism; it's accountability. Writing for an audience that can check my work keeps me honest
about what actually happened versus what makes a good story.

---

## The Takeaway Framework

If you're starting a complex AI-assisted project:

1. **State your mode** at the start of every session. Build, Improve, or Sweep. No mixing.

2. **Write reference docs** before implementing cross-cutting features. The AI will read them in every session,
   maintaining consistency you can't achieve through conversation alone.

3. **Create feature specs** with phased implementation plans. "Implement X" is too big. "Implement Phase 2 of X" is a
   session.

4. **Use gap documents** to convert "almost done" into "done." List what's missing, define success criteria, sweep
   systematically.

5. **Configure your agent** with AGENTS.md. Build commands, conventions, architecture overview. 30 minutes of writing
   saves hours of correction.

6. **Research before building** for novel features. Understand how others solved the problem before committing to an
   approach.

7. **Keep tests green.** Tests are your invariant across sessions. The AI can be aggressive because regressions get
   caught.

---

Token is MIT licensed and available at [github.com/HelgeSverre/token](https://github.com/HelgeSverre/token). The
conversation threads are at [ampcode.com/@helgesverre](https://ampcode.com/@helgesverre).

The editor is useful. The methodology is the interesting part.

---

*Helge Sverre is a developer based in Bergen, Norway. He builds things at [Crescat](https://crescat.io), runs too many
side projects, and occasionally writes about AI-assisted development. Find him
at [helgesver.re](https://helgesver.re) or [@helgesverre](https://x.com/HelgeSverre) on X.*