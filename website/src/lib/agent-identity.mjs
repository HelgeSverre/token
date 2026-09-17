/**
 * A small, deliberately conservative provider registry for archive presentation.
 * Unknown providers retain their recorded name and never receive an inferred mark.
 */
export const AGENT_IDENTITIES = Object.freeze({
  codex: Object.freeze({name: 'Codex', icon: 'codex'}),
  claude: Object.freeze({name: 'Claude', icon: 'claude'}),
  amp: Object.freeze({name: 'Amp', icon: 'amp'}),
  opencode: Object.freeze({name: 'OpenCode', icon: 'opencode'}),
  kimi: Object.freeze({name: 'Kimi CLI', icon: 'kimi'}),
  devin: Object.freeze({name: 'Devin', icon: 'devin'}),
  'antigravity-cli': Object.freeze({name: 'Antigravity', icon: 'antigravity'}),
});

const ALIASES = {'kimi-cli':'kimi', 'antigravity':'antigravity-cli', 'antigravity-acp':'antigravity-cli', 'claude-code':'claude'};

export function agentKey(agent) {
  const recorded = String(agent || '').trim().toLowerCase();
  const value = ALIASES[recorded] || recorded;
  return Object.hasOwn(AGENT_IDENTITIES, value) ? value : null;
}

export function agentName(agent) {
  const key = agentKey(agent);
  if (key) return AGENT_IDENTITIES[key].name;
  const recorded = String(agent || '').trim();
  return recorded || 'Agent';
}

export function agentIconName(agent) {
  const key = agentKey(agent);
  return key ? AGENT_IDENTITIES[key].icon : null;
}
