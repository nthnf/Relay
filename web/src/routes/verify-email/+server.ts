import { error, redirect } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';
import { setAuthCookies } from '$lib/server/auth-cookies';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const token = url.searchParams.get('token')?.trim();

	if (!token) {
		error(400, 'token is required');
	}

	try {
		const tokenPair = await getIdentityClient().redeemEmailVerificationToken({ token });
		setAuthCookies(cookies, tokenPair);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}

	redirect(303, '/profile');
};
