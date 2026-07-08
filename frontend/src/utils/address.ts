/** Parse `name#hash@hostname` into `{ name, hash, hostname }`. */
export function parseAddress(addr: string): { name: string; hash: string; hostname: string } {
  const at = addr.lastIndexOf("@");
  if (at < 0) return { name: addr, hash: "", hostname: "" };
  const hostname = addr.slice(at + 1);
  const local = addr.slice(0, at);
  const hashIdx = local.indexOf("#");
  if (hashIdx < 0) return { name: local, hash: "", hostname };
  return {
    name: local.slice(0, hashIdx),
    hash: local.slice(hashIdx + 1),
    hostname,
  };
}

/** Extract the hostname part from an address (after the last `@`). */
export function hostnameFromAddr(addr: string): string {
  return parseAddress(addr).hostname;
}

/** Extract the name part from an address (before the first `#`). */
export function nameFromAddr(addr: string): string {
  const hashIdx = addr.indexOf("#");
  return hashIdx >= 0 ? addr.slice(0, hashIdx) : addr;
}
