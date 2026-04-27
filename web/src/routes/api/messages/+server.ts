import { error, json } from '@sveltejs/kit';

import { getChatClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url, request, cookies }) => {
	const conversationId = url.searchParams.get('conversationId');

	if (!conversationId) {
		error(400, 'conversationId is required');
	}

	const pageSize = optionalInteger(url.searchParams.get('pageSize'));
	const beforeConversationMessageSeq = optionalInteger(
		url.searchParams.get('beforeConversationMessageSeq')
	);

	try {
		const response = await getChatClient().listConversationMessages(
			{
				conversationId,
				pageSize,
				beforeConversationMessageSeq
			},
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const POST: RequestHandler = async ({ request, cookies }) => {
	try {
		const body = await request.json();
		const response = await getChatClient().createMessage(body, {
			metadata: metadataFromRequest(request.headers, cookies)
		});

		return json(response, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

function optionalInteger(value: string | null): number | undefined {
	if (!value) {
		return undefined;
	}

	const parsed = Number(value);
	return Number.isInteger(parsed) ? parsed : undefined;
}
