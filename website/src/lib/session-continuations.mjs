// Amp writes a very specific handoff preamble into the first human message of
// a continued thread.  Only that preamble establishes an archive relationship:
// ordinary `read_thread` mentions are evidence inside a conversation, not a
// continuation edge.
const THREAD_ID = 'T-(?:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})';
const HANDOFF = new RegExp(
  String.raw`^\s*Continuing work from thread\s+(${THREAD_ID})\.\s*When you lack specific information you can use read_thread to get it\.(?:\s|$)`,
  'im',
);

export function canonicalAmpThreadId(value) {
  if (typeof value !== 'string') return null;
  const match = value.trim().match(new RegExp(String.raw`^(?:amp:)?(${THREAD_ID})$`, 'i'));
  return match ? `T-${match[1].slice(2).toLowerCase()}` : null;
}

function isAmp(session) {
  return String(session?.agent || '').toLowerCase() === 'amp';
}

/**
 * Return a continuation reference only when Amp's handoff boilerplate appears
 * in the opening human message. This deliberately ignores later prose and
 * tool calls that happen to name another thread.
 */
export function ampContinuationReference(session) {
  if (!isAmp(session) || !canonicalAmpThreadId(session.id)) return null;
  const openingHuman = (session.messages || []).find(message =>
    message?.role === 'user' && typeof message.text === 'string' && message.text.trim(),
  );
  if (!openingHuman) return null;
  const match = openingHuman.text.match(HANDOFF);
  if (!match) return null;
  return {
    threadId: canonicalAmpThreadId(match[1]),
    evidenceMessageId: openingHuman.id || null,
  };
}

function relation(reference = null) {
  return {
    predecessorId: null,
    successorIds: [],
    referenceThreadId: reference?.threadId || null,
    evidenceMessageId: reference?.evidenceMessageId || null,
    status: reference ? 'unresolved' : 'none',
  };
}

/**
 * Build a conservative directed graph of recorded Amp handoffs.
 *
 * A predecessor can have several successors (a legitimate fork). Canonical
 * ID collisions and cycles are retained as reviewable relation states rather
 * than being guessed into a navigable chain.
 */
export function buildContinuationIndex(sessions) {
  const byId = new Map(sessions.map(session => [session.id, session]));
  const canonicalIds = new Map();
  const relations = new Map();

  for (const session of sessions) {
    if (!isAmp(session)) continue;
    const canonical = canonicalAmpThreadId(session.id);
    if (!canonical) continue;
    const ids = canonicalIds.get(canonical) || [];
    ids.push(session.id);
    canonicalIds.set(canonical, ids);
    relations.set(session.id, relation(ampContinuationReference(session)));
  }

  for (const [sessionId, current] of relations) {
    if (!current.referenceThreadId) continue;
    const candidates = canonicalIds.get(current.referenceThreadId) || [];
    if (candidates.length === 1) {
      const predecessorId = candidates[0];
      if (predecessorId === sessionId) {
        current.status = 'cycle';
      } else {
        current.predecessorId = predecessorId;
        current.status = 'resolved';
      }
    } else if (candidates.length > 1) {
      current.status = 'ambiguous';
    }
  }

  // Remove every edge participating in a cycle. Edges entering a cycle stay
  // useful, but traversal terminates at the first cycle member.
  const cycleMembers = new Set();
  for (const start of relations.keys()) {
    const seen = new Map();
    let current = start;
    while (current && relations.get(current)?.predecessorId) {
      if (seen.has(current)) {
        for (const id of [...seen.keys()].slice(seen.get(current))) cycleMembers.add(id);
        break;
      }
      seen.set(current, seen.size);
      current = relations.get(current).predecessorId;
    }
  }
  for (const id of cycleMembers) {
    const current = relations.get(id);
    current.predecessorId = null;
    current.status = 'cycle';
  }

  for (const [id, current] of relations) {
    if (current.predecessorId && relations.has(current.predecessorId)) {
      relations.get(current.predecessorId).successorIds.push(id);
    }
  }
  for (const current of relations.values()) current.successorIds.sort((a, b) => a.localeCompare(b));

  return {byId, relations};
}

function connectedComponent(index, sessionId) {
  const visited = new Set([sessionId]);
  const pending = [sessionId];
  while (pending.length) {
    const id = pending.shift();
    const current = index.relations.get(id);
    const adjacent = [current?.predecessorId, ...(current?.successorIds || [])].filter(Boolean);
    for (const candidate of adjacent) {
      if (!visited.has(candidate)) {
        visited.add(candidate);
        pending.push(candidate);
      }
    }
  }
  return [...visited];
}

function linearChain(index, component) {
  if (component.length < 2) return null;
  const roots = component.filter(id => !index.relations.get(id)?.predecessorId);
  if (roots.length !== 1 || component.some(id => index.relations.get(id)?.successorIds.length > 1)) return null;
  const chain = [];
  const seen = new Set();
  let current = roots[0];
  while (current && !seen.has(current)) {
    seen.add(current);
    chain.push(current);
    current = index.relations.get(current)?.successorIds[0] || null;
  }
  return chain.length === component.length ? chain : null;
}

/**
 * Presentation-ready, safe context for one session. It exposes only explicit
 * adjacent links, while adding an ordinal only for an unbranched chain.
 */
export function continuationContext(index, sessionId) {
  const current = index.relations.get(sessionId);
  if (!current) return null;
  const predecessor = current.predecessorId ? index.byId.get(current.predecessorId) || null : null;
  const successors = current.successorIds.map(id => index.byId.get(id)).filter(Boolean);
  const component = connectedComponent(index, sessionId);
  const chain = linearChain(index, component);
  return {
    ...current,
    predecessor,
    successors,
    chain: chain?.map(id => index.byId.get(id)).filter(Boolean) || null,
    position: chain ? chain.indexOf(sessionId) + 1 : null,
    total: chain?.length || null,
  };
}
