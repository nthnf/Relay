import { error, fail, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getChatClient, getIdentityClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId, encodeRouteId } from '$lib/server/route-ids';

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

		const authorProfiles = await loadAuthorProfiles(
			messages.messages.map((message) => message.authorUserId),
			metadata
		);

		return {
			workspace: workspaceBootstrap.workspace,
			workspaceRouteId: params.workspaceId,
			channel,
			channels: workspaceBootstrap.channels.map((item) => ({ ...item, routeId: encodeRouteId(item.channelId) })),
			messages,
			authorProfiles
		};
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
	},
	edit: async ({ params, request, cookies }) => {
		const form = await request.formData();
		const messageId = String(form.get('messageId') ?? '').trim();
		const newBody = String(form.get('newBody') ?? '').trim();

		if (!messageId || !newBody) {
			return fail(400, { editError: 'Message and body are required' });
		}

		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			await getChatClient().editMessage({ messageId, newBody }, { metadata });
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { editError: message });
		}

		redirect(303, `/workspace/${params.workspaceId}/${params.channelId}`);
	},
	delete: async ({ params, request, cookies }) => {
		const messageId = String((await request.formData()).get('messageId') ?? '').trim();

		if (!messageId) {
			return fail(400, { deleteError: 'Message is required' });
		}

		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			await getChatClient().deleteMessage({ messageId }, { metadata });
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { deleteError: message });
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

async function loadAuthorProfiles(
	authorUserIds: string[],
	metadata: ReturnType<typeof metadataFromRequest>
) {
	const userIds = [...new Set(authorUserIds.filter(Boolean))];

	if (userIds.length === 0) {
		return [];
	}

	const response = await getIdentityClient().getUsersByIds({ userIds }, { metadata });
	return response.users;
}
