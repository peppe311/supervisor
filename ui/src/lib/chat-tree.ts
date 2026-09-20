import type {TaskVerification} from './task-verification';

export interface ChatLineage {
  kind: 'source' | 'fork' | 'pending' | 'delegated';
  parentChatId: string | null;
  parentTitle: string | null;
  parentAvailable: boolean;
  childCount: number;
  delegatedCount?: number;
}
export interface ProjectChat {
  id: string; title: string; projectRoot: string;
  pinned?: boolean; archived?: boolean; agentActive?: boolean;
  needsAttention?: boolean; mutationLocked?: boolean; updatedAtMs?: number;
  verification?: TaskVerification|null;
  lineage?: ChatLineage | null;
}
export interface ChatTreeRow { chat: ProjectChat; depth: number; children: number; parent: string | null }

export function chatRelationship(chat: ProjectChat): string {
  const lineage = chat.lineage;
  if (!lineage) return '';
  const count = [lineage.childCount > 0 ? `${lineage.childCount} fork${lineage.childCount === 1 ? '' : 's'}` : '',
    lineage.delegatedCount ? `${lineage.delegatedCount} delegated` : ''].filter(Boolean).join(' · ');
  if (lineage.kind === 'source') return `${lineage.delegatedCount ? 'Supervisor' : 'Original'}${count ? ` · ${count}` : ''}`;
  const source = lineage.parentTitle || 'source not linked locally';
  return `${lineage.kind === 'pending' ? 'Fork unconfirmed' : lineage.kind === 'delegated' ? 'Delegated' : 'Fork'} · ${source}${count ? ` · ${count}` : ''}`;
}

/** Display-only forest. Parent identities come from Rust, never title matching.
 * Missing/filtered/archived/cross-project parents leave a labelled root. Cycles
 * are flattened rather than hiding records. No sorting of the underlying data.
 */
export function chatForest(chats: ProjectChat[], collapsed: ReadonlySet<string> = new Set()): ChatTreeRow[] {
  const byId = new Map(chats.map(chat => [chat.id, chat]));
  const parents = new Map<string, string>();
  for (const chat of chats) {
    const parent = chat.lineage?.parentChatId;
    if (!parent || parent === chat.id || !byId.has(parent) || byId.get(parent)?.projectRoot !== chat.projectRoot) continue;
    const seen = new Set([chat.id]);
    let ancestor: string | null | undefined = parent;
    while (ancestor && byId.has(ancestor) && !seen.has(ancestor)) {
      seen.add(ancestor); ancestor = byId.get(ancestor)?.lineage?.parentChatId;
    }
    if (!ancestor || !seen.has(ancestor)) parents.set(chat.id, parent);
  }
  const children = new Map<string | null, ProjectChat[]>();
  for (const chat of chats) {
    const parent = parents.get(chat.id) || null;
    const group = children.get(parent) || []; group.push(chat); children.set(parent, group);
  }
  const rows: ChatTreeRow[] = [];
  const stack = (children.get(null) || []).slice().reverse().map(chat => ({chat, depth:0}));
  while (stack.length) {
    const {chat, depth} = stack.pop()!;
    const descendants = children.get(chat.id) || [];
    rows.push({chat, depth, children:descendants.length, parent:parents.get(chat.id) || null});
    if (!collapsed.has(chat.id)) for (let i=descendants.length-1;i>=0;i--) stack.push({chat:descendants[i], depth:depth+1});
  }
  return rows;
}

export function chatAge(chat: ProjectChat, now=Date.now()): string {
  if (chat.needsAttention) return 'Needs attention';
  if (chat.agentActive) return 'Working';
  const elapsed = Math.max(0, now - (chat.updatedAtMs || now));
  if (elapsed < 60_000) return 'now';
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)}m`;
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h`;
  return `${Math.floor(elapsed / 86_400_000)}d`;
}
