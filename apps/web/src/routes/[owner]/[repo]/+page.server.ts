import { env } from '$env/dynamic/private';
import { DepoApiError, DepoClient } from '@depo/api-client';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, url }) => {
  const path = url.searchParams.get('path') ?? 'README.md';
  const ref = url.searchParams.get('ref') ?? undefined;
  const client = new DepoClient({
    baseUrl: env.DEPO_API_ORIGIN ?? 'http://127.0.0.1:3847',
    fetchImpl: fetch
  });

  try {
    return {
      owner: params.owner,
      repo: params.repo,
      path,
      view: await client.repos.view(params.owner, params.repo, { ref, path }),
      error: null
    };
  } catch (error) {
    if (error instanceof DepoApiError) {
      return {
        owner: params.owner,
        repo: params.repo,
        path,
        view: null,
        error: {
          code: error.code,
          message: error.message
        }
      };
    }

    throw error;
  }
};
