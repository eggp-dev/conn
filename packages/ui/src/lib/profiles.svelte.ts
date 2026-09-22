import type { ConnClient } from "../runtime/client";
export type Backend = "local" | "wsl" | "ssh" | "docker";
export type Shell = "posix" | "fish" | "power_shell" | "cmd" | "custom";
export type Profile = { id: string; name: string; enabled: boolean; backend: Backend; shell: Shell; program: string; args: string[]; cwd: string | null; env: Record<string, string>; target: string | null; port: number | null };
export type ProfileConfig = { version: number; revision: number; defaultProfile: string; profiles: Profile[] };
export type Availability = { available: boolean; message: string };
export type Catalog = { config: ProfileConfig; availability: Record<string, Availability> };
export function createProfiles(client: ConnClient) {
  const {invoke} = client;
  const profileState = $state({ catalog: null as Catalog | null });
  async function refreshProfiles() {
    profileState.catalog = await invoke<Catalog>("profiles_catalog");
    return profileState.catalog;
  }


  return { profileState, refreshProfiles };
}
