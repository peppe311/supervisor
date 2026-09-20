/** Trusted UI command construction, not instructions for a model or shell. */
export function renameCommand(name: string): string | null {
  const title = name.trim();
  return title && !/[\r\n\0]/.test(title) ? `/rename ${title}` : null;
}

export function mcpVerbose(argument: unknown): boolean {
  return argument === "verbose";
}
