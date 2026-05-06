import { fail, redirect } from '@sveltejs/kit';
import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch }) => {
  const client = createDepoClient(fetch);

  try {
    return {
      repos: await client.repos.list(),
      error: null
    };
  } catch (error) {
    if (error instanceof DepoApiError) {
      return {
        repos: [],
        error: { code: error.code, message: error.message }
      };
    }
    throw error;
  }
};

export const actions = {
  default: async ({ request, fetch }) => {
    const data = await request.formData();
    const owner = (data.get('owner') as string ?? '').trim();
    const name = (data.get('name') as string ?? '').trim();

    if (!owner || !name) {
      return fail(400, {
        values: { owner, name },
        error: 'Owner and name are required.'
      });
    }

    const client = createDepoClient(fetch);

    try {
      await client.repos.create({ owner, name });
    } catch (error) {
      if (error instanceof DepoApiError) {
        return fail(error.status >= 500 ? 502 : error.status, {
          values: { owner, name },
          error: error.message
        });
      }
      throw error;
    }

    redirect(303, `/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`);
  }
} satisfies Actions;
