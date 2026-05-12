import { error, json } from '@sveltejs/kit';

import { getChatClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const PATCH: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const body = (await request.json()) as { newBody?: string };
		const response = await getChatClient().editMessage(
			{ messageId: params.messageId, newBody: body.newBody ?? '' },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const DELETE: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const response = await getChatClient().deleteMessage(
			{ messageId: params.messageId },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
