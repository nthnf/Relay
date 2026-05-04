import { error, json } from '@sveltejs/kit';

import { getRealtimeClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ request, cookies, url }) => {
	const userIds = url.searchParams
		.getAll('userId')
		.flatMap((value) => value.split(','))
		.map((value) => value.trim())
		.filter(Boolean);

	if (userIds.length === 0) {
		return json({ users: [] });
	}

	try {
		const response = await getRealtimeClient().getUserPresence(
			{ userIds: [...new Set(userIds)] },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
