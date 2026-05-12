import { error, json } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';
import { refreshTokenCookieName, setAuthCookies, tokenPairJson } from '$lib/server/auth-cookies';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	const body = await requestBody(request);
	const refreshToken = body.refreshToken ?? cookies.get(refreshTokenCookieName());

	if (!refreshToken) {
		error(401, 'refresh token is required');
	}

	try {
		const tokenPair = await getIdentityClient().refreshSession({
			refreshToken,
			clientInstanceId: body.clientInstanceId
		});
		setAuthCookies(cookies, tokenPair);

		return json(tokenPairJson(tokenPair));
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

async function requestBody(request: Request): Promise<{ refreshToken?: string; clientInstanceId?: string }> {
	if (!request.headers.get('content-type')?.includes('application/json')) {
		return {};
	}

	return (await request.json()) as { refreshToken?: string; clientInstanceId?: string };
}
