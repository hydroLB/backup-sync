export type Step = 1 | 2 | 3 | 4;

export type DestinationStatus = {
  writable: boolean;
  free_bytes: number | null;
  message: string;
} | null;

