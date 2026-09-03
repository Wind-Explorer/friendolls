export type ServerConnectionType = "https" | "http" | "direct";

const SCHEME = /^(https?|wss?):\/\//i;

export function connectionTypeFromScheme(scheme: string): ServerConnectionType {
  return scheme.toLowerCase().startsWith("https") ||
    scheme.toLowerCase().startsWith("wss")
    ? "https"
    : "http";
}

export function splitServerAddress(address: string): {
  address: string;
  connectionType: ServerConnectionType;
} {
  const match = address.match(SCHEME);
  if (!match) return { address, connectionType: "direct" };
  return {
    address: address.slice(match[0].length),
    connectionType: connectionTypeFromScheme(match[1]),
  };
}

export function storedServerAddress(
  address: string,
  connectionType: ServerConnectionType,
): string {
  const host = address.trim();
  if (connectionType === "direct") return host;
  return `${connectionType}://${host}`;
}

export function containsScheme(address: string): boolean {
  return SCHEME.test(address.trim());
}

export function pastedServerAddress(value: string): {
  address: string;
  connectionType: ServerConnectionType;
  port: string;
} | null {
  const trimmed = value.trim();
  if (!SCHEME.test(trimmed)) return null;

  try {
    const url = new URL(trimmed);
    if (
      url.username ||
      url.password ||
      (url.pathname !== "/" && url.pathname !== "") ||
      url.search ||
      url.hash
    ) {
      return null;
    }
    return {
      address: url.hostname,
      connectionType: connectionTypeFromScheme(url.protocol),
      port: url.port,
    };
  } catch {
    return null;
  }
}
