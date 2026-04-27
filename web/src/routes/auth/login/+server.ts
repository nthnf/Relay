import { error, json } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';
import { setAuthCookies, tokenPairJson } from '$lib/server/auth-cookies';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	try {
		const body = await request.json();
		const tokenPair = await getIdentityClient().authenticatePassword(body);
		setAuthCookies(cookies, tokenPair);

		return json(tokenPairJson(tokenPair));
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
