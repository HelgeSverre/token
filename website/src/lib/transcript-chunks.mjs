/**
 * Transcript pages deliberately render a bounded first segment. The remaining
 * segments are static fragments fetched only after a reader asks for them.
 * Message ids are retained verbatim, so ordinal gaps do not affect deep links.
 */
export const TRANSCRIPT_CHUNK_SIZE = 100;

export function transcriptChunks(session, size = TRANSCRIPT_CHUNK_SIZE) {
  const messages = session.messages || [];
  const count = Math.max(1, Math.ceil(messages.length / size));
  const messagePages = Object.create(null);
  for (let index = 0; index < messages.length; index += 1) {
    const id = messages[index]?.id;
    if (id) messagePages[id] = Math.floor(index / size);
  }
  return {size, count, messagePages};
}

export function transcriptChunk(session, page, size = TRANSCRIPT_CHUNK_SIZE) {
  const index = Math.max(0, Number.parseInt(String(page), 10) || 0);
  const start = index * size;
  return {...session, messages: (session.messages || []).slice(start, start + size)};
}

// Astro static routes retain legal colons in provider-prefixed session IDs.
export function sessionRouteId(sessionId) {
  return encodeURIComponent(sessionId).replace(/%3A/gi, ':');
}

export function transcriptChunkPath(sessionId, page) {
  return `/agent-chunks/${sessionRouteId(sessionId)}/${page}`;
}
