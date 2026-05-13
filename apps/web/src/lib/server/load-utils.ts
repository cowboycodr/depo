import { DepoApiError } from '@depo/api-client';

export function extractApiError(error: unknown): { code: string; message: string } | null {
  if (error instanceof DepoApiError) {
    return { code: error.code, message: error.message };
  }
  if (error instanceof TypeError && error.message === 'fetch failed') {
    return {
      code: 'api_unavailable',
      message: 'Depo API is not reachable. Start pnpm dev:api and reload this page.'
    };
  }
  return null;
}
