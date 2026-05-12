import { error, json } from '@sveltejs/kit';

import { getChatClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	try {
		const body = await request.json();
		const response = await getChatClient().createConversation(body, {
			metadata: metadataFromRequest(request.headers, cookies)
		});

		return json(response, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
