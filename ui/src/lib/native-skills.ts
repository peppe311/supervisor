import type {SkillMetadata} from "../../../protocol/app-server/0.153.4/typescript/v2/SkillMetadata";
import type {SkillErrorInfo} from "../../../protocol/app-server/0.153.4/typescript/v2/SkillErrorInfo";
export type NativeSkill=Pick<SkillMetadata,"name"|"description"|"path"|"scope"|"enabled"|"pluginId">;
export type SelectedSkill=Pick<SkillMetadata,"name"|"path"> & {id:string;directory:string;current:boolean};
export interface NativeSkillsView {
  requestId:string;viewId:string;directory:string;items:NativeSkill[];errors:SkillErrorInfo[];
  current:boolean;loading:boolean;error:string|null;
}
