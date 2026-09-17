/**
 * Presentation-only workspace path normalization for Token's archive.
 *
 * The archive intentionally keeps the provider's original path in `path`.
 * These helpers only provide a stable label so sessions recorded from the old
 * rust-editor checkout and disposable worktrees read as one project.
 */

const ROOT_PATTERNS = [
  // Worktree patterns must precede the main checkout: both are descendants of
  // the current checkout path, but the worktree identifier is not useful in a
  // source-file label.
  {
    expression: /\/Users\/helge\/code\/token-editor\/\.claude\/worktrees\/[^/]+(?:\/|(?=$|[\s'"`)}\],;:]))/g,
    label: '.',
  },
  {
    expression: /\/Users\/helge\/\.codex\/worktrees\/[^/]+\/token-editor(?:\/|(?=$|[\s'"`)}\],;:]))/g,
    label: '.',
  },
  {expression: /\/Users\/helge\/code\/token-editor(?:\/|(?=$|[\s'"`)}\],;:]))/g, label: '.'},
  {expression: /\/Users\/helge\/code\/rust-editor(?:\/|(?=$|[\s'"`)}\],;:]))/g, label: '.'},
  {expression: /~\/code\/token-editor(?:\/|(?=$|[\s'"`)}\],;:]))/g, label: '.'},
  {expression: /~\/code\/rust-editor(?:\/|(?=$|[\s'"`)}\],;:]))/g, label: '.'},
];

const URI = /[A-Za-z][A-Za-z0-9+.-]*:\/\/[^\s<>'"`]+/g;

function withRelativeRoot(root, path) {
  const suffix = path.slice(root.length);
  return suffix.startsWith('/') ? suffix.slice(1) || '.' : '.';
}

/**
 * Returns a Token-project-relative display path when (and only when) `value`
 * is a whole path rooted in a known checkout/worktree. Other paths and URLs
 * are returned verbatim.
 */
export function canonicalizeWorkspacePath(value) {
  if (typeof value !== 'string' || !value) return value;
  if (/^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(value)) return value;
  for (const {expression} of ROOT_PATTERNS) {
    expression.lastIndex = 0;
    const match = expression.exec(value);
    if (match?.index === 0) return withRelativeRoot(match[0].replace(/\/$/, ''), value);
  }
  return value;
}

function normalizeNonUriText(text) {
  let result = text;
  for (const {expression, label} of ROOT_PATTERNS) {
    expression.lastIndex = 0;
    result = result.replace(expression, (root, offset, source) => {
      // Do not shorten a look-alike contained in a different filesystem path.
      const previous = source[offset - 1];
      if (previous && /[A-Za-z0-9._~\/-]/.test(previous)) return root;
      const following = source[offset + root.length];
      // Embedded absolute paths should read `src/lib.rs`, not `./src/lib.rs`.
      // A root on its own, including one with a trailing slash, still needs a
      // useful relative-path marker rather than disappearing from prose.
      const isDescendant = root.endsWith('/') && following && !/[\s'"`)}\],;:]/.test(following);
      return isDescendant ? '' : label;
    });
  }
  return result;
}

/**
 * Rewrites only known absolute/tilde workspace paths embedded in prose, JSON,
 * command output, and tool input. URI spans are deliberately left untouched.
 */
export function canonicalizeWorkspacePaths(value) {
  if (typeof value !== 'string' || !value) return value;
  let output = '';
  let cursor = 0;
  for (const match of value.matchAll(URI)) {
    output += normalizeNonUriText(value.slice(cursor, match.index));
    output += match[0];
    cursor = match.index + match[0].length;
  }
  return output + normalizeNonUriText(value.slice(cursor));
}

/** Adds a non-destructive display field while retaining the provider path. */
export function presentSessionFiles(files) {
  return (Array.isArray(files) ? files : []).map(file => ({
    ...file,
    displayPath: canonicalizeWorkspacePath(file?.path),
  }));
}
