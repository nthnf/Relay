import { error, json } from '@sveltejs/kit';

import { getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId } from '$lib/server/route-ids';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const workspaceId = decodeRouteId(params.workspaceId);
		const response = await getWorkspaceClient().createInviteLink(
			{ workspaceId },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
