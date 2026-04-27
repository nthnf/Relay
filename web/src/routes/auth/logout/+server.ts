import { error, json } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';
import { clearAuthCookies, sessionIdCookieName } from '$lib/server/auth-cookies';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	const sessionId = cookies.get(sessionIdCookieName());

	try {
		if (sessionId) {
			await getIdentityClient().revokeSession({ sessionId, revokeReason: 'logout' });
		}

		clearAuthCookies(cookies);
		return json({ revoked: Boolean(sessionId) });
	} catch (cause) {
		clearAuthCookies(cookies);
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
