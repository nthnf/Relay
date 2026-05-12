import { error, fail, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getChatClient, getIdentityClient, getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId, encodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, cookies, url }) => {
	const workspaceId = decodeParam(params.workspaceId);
	const channelId = decodeParam(params.channelId);
	const seededConversationId = url.searchParams.get('conversationId')?.trim();
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const workspaceBootstrap = await waitForWorkspaceChannel(workspaceId, channelId, metadata);
		let channel = workspaceBootstrap.channels.find((item) => item.channelId === channelId);
		let workspace = workspaceBootstrap.workspace;
		let channels = workspaceBootstrap.channels;

		if ((!workspace || !channel?.conversationId) && seededConversationId) {
			const fallback = await loadWorkspaceChannelFallback(workspaceId, channelId, seededConversationId, metadata);
			workspace = fallback.workspace;
			channel = fallback.channel;
			channels = fallback.channels;
		}

		if (!workspace || !channel?.conversationId) {
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
			workspace,
			workspaceRouteId: params.workspaceId,
			channel,
			channels: channels.map((item) => ({ ...item, routeId: encodeRouteId(item.channelId) })),
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

async function loadWorkspaceChannelFallback(
	workspaceId: string,
	channelId: string,
	conversationId: string,
	metadata: ReturnType<typeof metadataFromRequest>
) {
	const [workspaceDetails, channelList] = await Promise.all([
		getWorkspaceClient().getWorkspace({ workspaceId }, { metadata }),
		getWorkspaceClient().listChannels({ workspaceId }, { metadata })
	]);
	const channel = channelList.channels.find((item) => item.channelId === channelId);

	if (!channel) {
		error(404, 'Workspace channel not found');
	}

	return {
		workspace: {
			workspaceId: workspaceDetails.workspaceId,
			name: workspaceDetails.name,
			iconUrl: undefined,
			memberCount: workspaceDetails.memberCount,
			unreadCount: 0
		},
		channel: {
			channelId: channel.channelId,
			conversationId,
			name: channel.name,
			channelKind: channel.channelKind,
			position: channel.position,
			unreadCount: 0
		},
		channels: channelList.channels.map((item) => ({
			channelId: item.channelId,
			conversationId: item.channelId === channelId ? conversationId : '',
			name: item.name,
			channelKind: item.channelKind,
			position: item.position,
			unreadCount: 0
		}))
	};
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

async function waitForWorkspaceChannel(
	workspaceId: string,
	channelId: string,
	metadata: ReturnType<typeof metadataFromRequest>
) {
	for (let attempt = 0; attempt < 30; attempt += 1) {
		const latest = await getBootstrapClient().getWorkspaceBootstrap({ workspaceId }, { metadata });
		const channel = latest.channels.find((item) => item.channelId === channelId);

		if (channel?.conversationId) {
			return latest;
		}

		await new Promise((resolve) => setTimeout(resolve, 250));
	}

	return getBootstrapClient().getWorkspaceBootstrap({ workspaceId }, { metadata });
}
