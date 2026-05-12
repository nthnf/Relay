import { error, json } from '@sveltejs/kit';

import {
	getBootstrapClient,
	getChatClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ request, cookies }) => {
	try {
		const response = await getBootstrapClient().getDmBootstrap(
			{},
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
		const peerUserId = String(body.peerUserId ?? '').trim();

		if (!peerUserId) {
			error(400, 'peerUserId is required');
		}

		const metadata = metadataFromRequest(request.headers, cookies);
		const conversation = await getChatClient().createConversation(
			{ targetType: 1, peerUserId },
			{ metadata }
		);
		const bootstrap = await getBootstrapClient().getDmBootstrap({}, { metadata });
		const thread = bootstrap.items.find((item) => item.peerUserId === peerUserId);

		return json({ conversation, thread }, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
