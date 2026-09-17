import {sessions} from '../lib/agent-session-data';
import {buildSessionIndex} from '../lib/session-index.mjs';

const index = buildSessionIndex(sessions);

export function GET() {
  return new Response(JSON.stringify(index), {
    headers: {
      'content-type': 'application/json; charset=utf-8',
      // The document is generated at build time. Revalidate it on a new visit
      // instead of pinning an older deployment's catalogue in the browser.
      'cache-control': 'no-cache',
    },
  });
}
