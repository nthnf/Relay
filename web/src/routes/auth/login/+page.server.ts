import { fail, redirect } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';
import { setAuthCookies } from '$lib/server/auth-cookies';

import type { Actions } from './$types';

export const actions: Actions = {
	signin: async ({ request, cookies }) => {
		const formData = await request.formData();
		const email = String(formData.get('email') ?? '').trim();
		const password = String(formData.get('password') ?? '');

		if (!email || !password) {
			return fail(400, { email, error: 'Email and password are required' });
		}

		try {
			const tokenPair = await getIdentityClient().authenticatePassword({ email, password });
			setAuthCookies(cookies, tokenPair);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { email, error: message });
		}

		redirect(303, '/');
	}
};
