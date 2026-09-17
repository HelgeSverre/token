import {readFileSync, realpathSync} from 'node:fs';
import {resolve, join} from 'node:path';

/** Private data is explicitly opted into for local development only. */
export function privateArchiveRoot(env = process.env) {
  return env.AGENT_ARCHIVE_PREVIEW ? realpathSync(resolve(env.AGENT_ARCHIVE_PREVIEW)) : null;
}

export function readPrivateArchive(root) {
  const receipt = JSON.parse(readFileSync(join(root, 'index.json'), 'utf8'));
  if (receipt.purpose !== 'private-preview') throw new Error('Expected a private-preview archive receipt');
  if (!Array.isArray(receipt.sessions)) throw new Error('Missing preview session manifest');
  const ids = new Set();
  return Object.fromEntries(receipt.sessions.map(record => {
    if (!/^sessions\/[a-f0-9]{64}\.json$/.test(record.path)) throw new Error('Invalid preview session path');
    const bytes = readFileSync(join(root, record.path), 'utf8');
    const session = JSON.parse(bytes);
    if (session.id !== record.id || ids.has(session.id)) throw new Error('Invalid or duplicate preview identity');
    ids.add(session.id);
    return [record.path, bytes];
  }));
}
