import { error, fail, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getChatClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, cookies }) => {
	const workspaceId = decodeParam(params.workspaceId);
	const channelId = decodeParam(params.channelId);
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const workspaceBootstrap = await getBootstrapClient().getWorkspaceBootstrap(
			{ workspaceId },
			{ metadata }
		);
		const channel = workspaceBootstrap.channels.find((item) => item.channelId === channelId);

		if (!workspaceBootstrap.workspace || !channel) {
			error(404, 'Workspace channel not found');
		}

		const messages = await getChatClient().listConversationMessages(
			{ conversationId: channel.conversationId, pageSize: 50 },
			{ metadata }
		);

		return { workspace: workspaceBootstrap.workspace, channel, messages };
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

		const workspaceId = decodeParam(params.workspaceId);
		const channelId = decodeParam(params.channelId);
		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			const workspaceBootstrap = await getBootstrapClient().getWorkspaceBootstrap(
				{ workspaceId },
				{ metadata }
			);
			const channel = workspaceBootstrap.channels.find((item) => item.channelId === channelId);

			if (!channel) {
				error(404, 'Workspace channel not found');
			}

			await getChatClient().createMessage({ conversationId: channel.conversationId, body }, { metadata });
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { body, error: message });
		}

		redirect(303, `/workspace/${params.workspaceId}/${params.channelId}`);
	}
};

function decodeParam(value: string): string {
	try {
		return decodeRouteId(value);
	} catch {
		error(400, 'Invalid route id');
	}
}
