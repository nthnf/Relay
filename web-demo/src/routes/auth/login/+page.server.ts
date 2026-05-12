import { fail, redirect } from '@sveltejs/kit';

import { setAuthCookies } from '$lib/server/auth-cookies';
import { demoTokenPair } from '$lib/server/demo-data';

import type { Actions } from './$types';

export const actions: Actions = {
	signin: async ({ request, cookies }) => {
		const formData = await request.formData();
		const email = String(formData.get('email') ?? '').trim();
		const password = String(formData.get('password') ?? '');

		if (!email || !password) {
			return fail(400, { email, error: 'Email and password are required' });
		}

		setAuthCookies(cookies, demoTokenPair());

		redirect(303, '/');
	}
};
