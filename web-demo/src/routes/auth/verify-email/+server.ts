import { error, json } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';
import { setAuthCookies, tokenPairJson } from '$lib/server/auth-cookies';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	const body = (await request.json()) as { token?: string };

	if (!body.token) {
		error(400, 'token is required');
	}

	try {
		const tokenPair = await getIdentityClient().redeemEmailVerificationToken({ token: body.token });
		setAuthCookies(cookies, tokenPair);

		return json(tokenPairJson(tokenPair));
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
