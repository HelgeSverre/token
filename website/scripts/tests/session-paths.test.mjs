import test from 'node:test';
import assert from 'node:assert/strict';
import {
  canonicalizeWorkspacePath,
  canonicalizeWorkspacePaths,
  presentSessionFiles,
} from '../../src/lib/session-paths.mjs';

test('known historical checkout roots and disposable worktrees become relative labels', () => {
  const cases = [
    ['/Users/helge/code/token-editor/src/runtime/app.rs', 'src/runtime/app.rs'],
    ['/Users/helge/code/rust-editor/src/runtime/app.rs', 'src/runtime/app.rs'],
    ['~/code/token-editor/Cargo.toml', 'Cargo.toml'],
    ['~/code/rust-editor/Cargo.toml', 'Cargo.toml'],
    ['/Users/helge/.codex/worktrees/c90f/token-editor/website/src/pages/agent.astro', 'website/src/pages/agent.astro'],
    ['/Users/helge/code/token-editor/.claude/worktrees/agent-a1534efd7fda00ed6/src/update/lsp.rs', 'src/update/lsp.rs'],
    ['/Users/helge/code/token-editor', '.'],
  ];
  for (const [source, expected] of cases) assert.equal(canonicalizeWorkspacePath(source), expected);
});

test('only exact workspace roots are shortened', () => {
  const unrelated = [
    '/Users/helge/code/token-editor-backup/src/main.rs',
    '/Users/helge/code/other-token-editor/src/main.rs',
    '/private/tmp/claude-501/-Users-helge-code-token-editor/session/tasks/result.output',
    '/Users/helge/code/wip-phpantom-lsp-bugs/src/Report.php',
  ];
  for (const source of unrelated) assert.equal(canonicalizeWorkspacePath(source), source);
});

test('embedded paths are normalized without rewriting URLs or look-alikes', () => {
  const source = [
    'Read: /Users/helge/code/rust-editor/src/main.rs',
    ' and ~/code/token-editor/Cargo.toml',
    ' https://example.test/Users/helge/code/token-editor/src/main.rs',
    ' file:///Users/helge/code/token-editor/src/main.rs',
    ' /private/tmp/claude-501/-Users-helge-code-token-editor/task.output',
  ].join('');
  assert.equal(
    canonicalizeWorkspacePaths(source),
    'Read: src/main.rs and Cargo.toml https://example.test/Users/helge/code/token-editor/src/main.rs file:///Users/helge/code/token-editor/src/main.rs /private/tmp/claude-501/-Users-helge-code-token-editor/task.output',
  );
});

test('embedded exact roots survive JSON punctuation and trailing slashes', () => {
  const source = [
    '{"cwd":"/Users/helge/code/token-editor"}',
    " legacy='~/code/rust-editor'",
    ' worktree=/Users/helge/.codex/worktrees/c90f/token-editor/; ',
    ' lookalike=/Users/helge/code/token-editor-backup',
  ].join('');
  assert.equal(
    canonicalizeWorkspacePaths(source),
    '{"cwd":"."} legacy=\'.\' worktree=.;  lookalike=/Users/helge/code/token-editor-backup',
  );
});

test('session file presentation preserves original paths and evidence', () => {
  const [file] = presentSessionFiles([{
    path: '/Users/helge/code/token-editor/src/lib.rs',
    evidenceMessageIds: ['ordinal:7'],
    kind: 'modified',
  }]);
  assert.equal(file.path, '/Users/helge/code/token-editor/src/lib.rs');
  assert.equal(file.displayPath, 'src/lib.rs');
  assert.deepEqual(file.evidenceMessageIds, ['ordinal:7']);
});
