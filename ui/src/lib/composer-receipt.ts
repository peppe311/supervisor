export interface ComposerReceipt {
  owner: string;
  input: string;
}

/** A delayed acknowledgement may never consume an identical draft in another chat. */
export function acceptsComposerReceipt(detail: unknown, owner: string, input: string): boolean {
  if (!detail || typeof detail !== "object" || !owner) return false;
  const receipt = detail as Partial<ComposerReceipt>;
  return receipt.owner === owner && typeof receipt.input === "string"
    && receipt.input.trim() === input.trim();
}
