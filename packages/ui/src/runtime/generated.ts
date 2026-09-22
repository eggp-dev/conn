// Generated from conn-frontend owner DTOs. Do not edit.
// Run: node scripts/generate-owner-contract.mjs [--check]

export const OWNER_PROTOCOL = 1 as const;

export type OwnerHello = {
  "buildVersion": string;
  "epoch": number;
  "protocol": number;
  "runtimeId": string;
  "viewId": string;
};

export type OwnerMeta = {
  "epoch": number;
  "operationId": number;
  "sequence"?: number | null;
};

export type OwnerOutput = {
  "data": string;
  "epoch": number;
  "generation": number;
  "outputSeq": number;
  "reset"?: boolean;
  "session": string;
  "size": OwnerSize;
  "streamSeq": number;
};

export type OwnerRequest = {
  "args"?: unknown;
  "epoch": number;
  "name": string;
  "operationId": number;
  "sequence"?: number | null;
};

export type OwnerSize = {
  "cols": number;
  "rows": number;
};
