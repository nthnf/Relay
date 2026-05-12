import { error, redirect } from '@sveltejs/kit';

import { getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId, encodeRouteId } from '$lib/server/route-ids';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params, request, cookies }) => {
	const workspaceId = decodeParam(params.workspaceId);

	try {
		const channels = await getWorkspaceClient().listChannels(
			{ workspaceId },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);
		const firstChannel = channels.channels[0];

		if (!firstChannel) {
			error(404, 'Workspace has no channels');
		}

		redirect(303, `/workspace/${params.workspaceId}/${encodeRouteId(firstChannel.channelId)}`);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

function decodeParam(value: string): string {
	try {
		return decodeRouteId(value);
	} catch {
		error(400, 'Invalid route id');
	}
}
