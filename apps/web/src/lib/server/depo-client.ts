import { env } from '$env/dynamic/private';
import { DepoClient } from '@depo/api-client';

export function createDepoClient(fetchImpl: typeof fetch) {
  return new DepoClient({
    baseUrl: env.DEPO_API_ORIGIN ?? 'http://127.0.0.1:3847',
    fetchImpl
  });
}
