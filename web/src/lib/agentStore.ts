// 에이전트 목록을 localStorage에 저장하는 간단한 스토어.
// 각 에이전트는 고유 sessionId를 가져 대화 히스토리가 독립된다.

import { generateUUID } from './uuid';

export type AgentColor = 'accent' | 'iris' | 'spring' | 'sakura' | 'carp' | 'wave';

// AutonomyConfig 기반 에이전트별 권한 정책
export interface AgentPolicy {
  workingDir: string;           // 기본 작업 디렉토리
  allowedRoots: string[];       // 추가 접근 허용 폴더
  allowedCommands: string[];    // 허용 명령어 목록 (빈 배열 = 제한 없음)
  autoApproveTools: string[];   // 자동 승인 도구
  alwaysAskTools: string[];     // 항상 확인 도구
  systemPrompt: string;         // 사용자 정의 시스템 프롬프트
  shellTimeoutSecs: number;     // 쉘 타임아웃 (초)
  workspaceOnly: boolean;       // 워크스페이스 내부만 접근
}

export function defaultPolicy(): AgentPolicy {
  return {
    workingDir: '',
    allowedRoots: [],
    allowedCommands: [],
    autoApproveTools: ['file_read', 'memory_recall', 'web_search_tool', 'web_fetch', 'calculator', 'glob_search'],
    alwaysAskTools: [],
    systemPrompt: '',
    shellTimeoutSecs: 60,
    workspaceOnly: true,
  };
}

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
  policy: AgentPolicy;
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
    const raw = JSON.parse(localStorage.getItem(STORE_KEY) ?? '[]') as Agent[];
    // 구버전 데이터에 policy 필드 없을 경우 기본값 병합
    return raw.map(a => ({ ...a, policy: a.policy ?? defaultPolicy() }));
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
    policy: defaultPolicy(),
  };
  save([...agents, agent]);
  return agent;
}

export function deleteAgent(id: string) {
  save(load().filter(a => a.id !== id));
}

export function updateAgent(id: string, patch: Partial<Pick<Agent, 'name' | 'description' | 'color' | 'messageCount' | 'lastMessageAt' | 'provider' | 'model'>>) {
  save(load().map(a => a.id === id ? { ...a, ...patch } : a));
}

export function updateAgentPolicy(id: string, policy: AgentPolicy) {
  save(load().map(a => a.id === id ? { ...a, policy } : a));
}

export function touchAgent(id: string, messageCount: number) {
  updateAgent(id, { messageCount, lastMessageAt: new Date().toISOString() });
}
