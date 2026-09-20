export type TaskVerificationState = 'working' | 'checking' | 'ready' | 'blocked';

export interface TaskVerification {
  state: TaskVerificationState;
  summary: string;
}

export function taskVerificationLabel(state:TaskVerificationState):string {
  return ({working:'Working',checking:'Checking',ready:'Ready',blocked:'Blocked'})[state];
}
