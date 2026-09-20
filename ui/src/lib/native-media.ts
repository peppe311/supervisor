export interface NativeMedia {
  state: "ready" | "unavailable";
  label: string;
  previewDataUrl?: string;
  width?: number;
  height?: number;
}

// Defense in depth: persisted or malformed host data never starts a fetch.
export function nativeMedia(value: unknown): NativeMedia | null {
  if (!value || typeof value !== "object") return null;
  const media = value as Partial<NativeMedia>;
  if (media.state === "unavailable") return {state: "unavailable", label: typeof media.label === "string" ? media.label : "Image preview unavailable."};
  if (media.state !== "ready") return null;
  const {previewDataUrl, width, height} = media;
  if (typeof previewDataUrl !== "string" || previewDataUrl.length > 24 * 1024 * 1024 + 32
      || !/^data:image\/(png|jpeg);base64,[A-Za-z0-9+/]+={0,2}$/.test(previewDataUrl)
      || typeof width !== "number" || typeof height !== "number"
      || !Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1
      || width > 16_384 || height > 16_384 || width * height > 64_000_000) {
    return {state: "unavailable", label: "Image preview unavailable. The original result remains in native Codex history."};
  }
  return {state: "ready", label: "Generated image", previewDataUrl, width, height};
}
