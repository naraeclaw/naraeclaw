// 에이전트 목록을 localStorage에 저장하는 간단한 스토어.
// 각 에이전트는 고유 sessionId를 가져 대화 히스토리가 독립된다.

import { generateUUID } from './uuid';

export type AgentColor = 'accent' | 'iris' | 'spring' | 'sakura' | 'carp' | 'wave';

export interface Agent {
  id: string;
  name: string;
  provider: string;
  model: string;
  color: AgentColor;
  description?: string;
  sessionId: string;     // 고정 UUID — chatHistoryStorage 키로 사용
  createdAt: string;
  lastMessageAt?: string;
  messageCount: number;
}

const STORE_KEY = 'naraeclaw_agents_v1';

export const COLOR_VARS: Record<AgentColor, string> = {
  accent: 'var(--pc-accent)',
  iris:   'var(--pc-iris)',
  spring: 'var(--pc-spring)',
  sakura: 'var(--pc-sakura)',
  carp:   'var(--pc-carp)',
  wave:   'var(--pc-wave)',
};

export const COLOR_LABELS: Record<AgentColor, string> = {
  accent: '스카이', iris: '보라', spring: '초록',
  sakura: '핑크',  carp: '황금', wave:   '하늘',
};

function load(): Agent[] {
  try {
    return JSON.parse(localStorage.getItem(STORE_KEY) ?? '[]');
  } catch { return []; }
}

function save(agents: Agent[]) {
  localStorage.setItem(STORE_KEY, JSON.stringify(agents));
}

export function listAgents(): Agent[] {
  return load();
}

export function getAgent(id: string): Agent | undefined {
  return load().find(a => a.id === id);
}

export function createAgent(fields: {
  id: string;
  name: string;
  provider: string;
  model: string;
  color: AgentColor;
  description?: string;
}): Agent {
  const agents = load();
  if (agents.find(a => a.id === fields.id)) throw new Error(`'${fields.id}' ID가 이미 존재합니다`);
  const agent: Agent = {
    ...fields,
    sessionId: generateUUID(),
    createdAt: new Date().toISOString(),
    messageCount: 0,
  };
  save([...agents, agent]);
  return agent;
}

export function deleteAgent(id: string) {
  save(load().filter(a => a.id !== id));
}

export function updateAgent(id: string, patch: Partial<Pick<Agent, 'name' | 'description' | 'color' | 'messageCount' | 'lastMessageAt'>>) {
  save(load().map(a => a.id === id ? { ...a, ...patch } : a));
}

export function touchAgent(id: string, messageCount: number) {
  updateAgent(id, { messageCount, lastMessageAt: new Date().toISOString() });
}
