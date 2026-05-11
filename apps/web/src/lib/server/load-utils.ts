import { DepoApiError } from '@depo/api-client';

export function extractApiError(error: unknown): { code: string; message: string } | null {
  if (error instanceof DepoApiError) {
    return { code: error.code, message: error.message };
  }
  return null;
}
