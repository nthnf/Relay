import { error, json } from '@sveltejs/kit';

import { getChatClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const body = (await request.json()) as { lastReadConversationMessageSeq?: number };
		const response = await getChatClient().markConversationRead(
			{
				conversationId: params.conversationId,
				lastReadConversationMessageSeq: body.lastReadConversationMessageSeq ?? 0
			},
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
