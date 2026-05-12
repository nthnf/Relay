import { error, json } from '@sveltejs/kit';

import { getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId } from '$lib/server/route-ids';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const workspaceId = decodeRouteId(params.workspaceId);
		const body = (await request.json()) as { targetUserId?: string };
		const targetUserId = body.targetUserId?.trim();

		if (!targetUserId) {
			error(400, 'targetUserId is required');
		}

		const response = await getWorkspaceClient().addMember(
			{ workspaceId, targetUserId },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const GET: RequestHandler = async ({ params, request, cookies, url }) => {
	try {
		const workspaceId = decodeRouteId(params.workspaceId);
		const pageSize = optionalInteger(url.searchParams.get('pageSize')) ?? 200;
		const pageToken = url.searchParams.get('pageToken') ?? undefined;
		const response = await getWorkspaceClient().listWorkspaceMembers(
			{ workspaceId, pageSize, pageToken },
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
		const workspaceId = decodeRouteId(params.workspaceId);
		const body = (await request.json()) as { targetUserId?: string };
		const targetUserId = body.targetUserId?.trim();

		if (!targetUserId) {
			error(400, 'targetUserId is required');
		}

		const response = await getWorkspaceClient().removeMember(
			{ workspaceId, targetUserId },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
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
