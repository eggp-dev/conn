import brand from "@conn/brand/brand.json";

/**
 * The films name the repository from @conn/brand, shared with the site and the release tooling.
 * If it moves, change it there and re-render; nothing in media/demo spells the address out.
 * GitHub keeps redirecting the old address after a transfer, so already published films stay valid.
 */
export const repository = `github.com/${brand.repository}`;

/** Agent clients Conn sets up today, as shown in the closing card. */
export const worksWith: readonly string[] = brand.worksWith;
