import { error, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { encodeRouteId } from '$lib/server/route-ids';

import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, cookies }) => {
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const joined = await getWorkspaceClient().joinWorkspaceByInviteLink(
			{ code: params.code },
			{ metadata }
		);
		const workspaceRouteId = encodeRouteId(joined.workspaceId);
		const bootstrap = await getBootstrapClient().getWorkspaceBootstrap(
			{ workspaceId: joined.workspaceId },
			{ metadata }
		);
		const firstChannel = bootstrap.channels[0];

		if (firstChannel) {
			redirect(303, `/workspace/${workspaceRouteId}/${encodeRouteId(firstChannel.channelId)}`);
		}

		redirect(303, `/workspace/${workspaceRouteId}`);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
