import { error, fail, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getChatClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, cookies }) => {
	const dmPairId = decodeParam(params.dmPairId);
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const dmBootstrap = await getBootstrapClient().getDmBootstrap({}, { metadata });
		const thread = dmBootstrap.items.find((item) => item.dmPairId === dmPairId);

		if (!thread) {
			error(404, 'DM thread not found');
		}

		const messages = await getChatClient().listConversationMessages(
			{ conversationId: thread.conversationId, pageSize: 50 },
			{ metadata }
		);

		return { thread, messages };
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const actions: Actions = {
	send: async ({ params, request, cookies }) => {
		const body = String((await request.formData()).get('body') ?? '').trim();

		if (!body) {
			return fail(400, { body, error: 'Message body is required' });
		}

		const dmPairId = decodeParam(params.dmPairId);
		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			const dmBootstrap = await getBootstrapClient().getDmBootstrap({}, { metadata });
			const thread = dmBootstrap.items.find((item) => item.dmPairId === dmPairId);

			if (!thread) {
				error(404, 'DM thread not found');
			}

			await getChatClient().createMessage({ conversationId: thread.conversationId, body }, { metadata });
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { body, error: message });
		}

		redirect(303, `/dm/${params.dmPairId}`);
	}
};

function decodeParam(value: string): string {
	try {
		return decodeRouteId(value);
	} catch {
		error(400, 'Invalid route id');
	}
}
