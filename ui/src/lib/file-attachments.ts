export interface FileAttachment {
  id: string; name: string; kind: "text" | "image" | "audio"; iconKey?: string;
  byteCount?: number; estimatedTokenCount?: number; redactionCount?: number;
  previewDataUrl?: string;
  sourceLabel?: string;
}

// Drafts and stored histories can contain legacy data. Never fetch a remote
// preview URL or interpolate an arbitrary URI from native history.
export function attachmentPreview(value: unknown): string | undefined {
  return typeof value === "string" && /^data:image\/(png|jpeg);base64,[A-Za-z0-9+/=]+$/.test(value) ? value : undefined;
}
export function attachmentList(value: unknown): FileAttachment[] {
  if (!Array.isArray(value)) return [];
  return value.filter((file): file is FileAttachment => Boolean(file && typeof file === "object" && typeof file.id === "string" && typeof file.name === "string" && ["text","image","audio"].includes(file.kind)));
}
