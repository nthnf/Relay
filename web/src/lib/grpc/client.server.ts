import { env } from '$env/dynamic/private';
import { Metadata, Status, createChannel, createClient, type ClientError } from 'nice-grpc';
import { isHttpError, isRedirect, type Cookies } from '@sveltejs/kit';

import { BootstrapServiceDefinition } from '../../generated/bootstrap';
import { ChatServiceDefinition, type ChatServiceClient } from '../../generated/chat';
import { FriendshipServiceDefinition, type FriendshipServiceClient } from '../../generated/friendship';
import { IdentityServiceDefinition, type IdentityServiceClient } from '../../generated/identity';
import { RealtimeServiceDefinition, type RealtimeServiceClient } from '../../generated/realtime';
import { WorkspaceServiceDefinition, type WorkspaceServiceClient } from '../../generated/workspace';
import type { BootstrapServiceClient } from '../../generated/bootstrap';

const serviceDefinitions = {
	bootstrap: BootstrapServiceDefinition,
	chat: ChatServiceDefinition,
	friendship: FriendshipServiceDefinition,
	identity: IdentityServiceDefinition,
	realtime: RealtimeServiceDefinition,
	workspace: WorkspaceServiceDefinition
} as const;

export type ServiceName = keyof typeof serviceDefinitions;

const serviceEnvNames: Record<ServiceName, string> = {
	bootstrap: 'BOOTSTRAP_GRPC_TARGET',
	chat: 'CHAT_GRPC_TARGET',
	friendship: 'FRIENDSHIP_GRPC_TARGET',
	identity: 'IDENTITY_GRPC_TARGET',
	realtime: 'REALTIME_GRPC_TARGET',
	workspace: 'WORKSPACE_GRPC_TARGET'
};

const serviceAuthorityEnvNames: Record<ServiceName, string> = {
	bootstrap: 'BOOTSTRAP_GRPC_AUTHORITY',
	chat: 'CHAT_GRPC_AUTHORITY',
	friendship: 'FRIENDSHIP_GRPC_AUTHORITY',
	identity: 'IDENTITY_GRPC_AUTHORITY',
	realtime: 'REALTIME_GRPC_AUTHORITY',
	workspace: 'WORKSPACE_GRPC_AUTHORITY'
};

const clients = new Map<string, unknown>();

export function isServiceName(value: string): value is ServiceName {
	return value in serviceDefinitions;
}

export function getGrpcClient(service: ServiceName): Record<string, GrpcUnaryMethod> {
	const target = env[serviceEnvNames[service]] ?? env.GRPC_TARGET;

	if (!target) {
		throw new Error(`Missing ${serviceEnvNames[service]} or GRPC_TARGET for ${service} gRPC client`);
	}

	const authority = env[serviceAuthorityEnvNames[service]] ?? env.GRPC_AUTHORITY ?? `${service}.local`;
	const cacheKey = `${service}:${target}:${authority ?? ''}`;
	const cached = clients.get(cacheKey);

	if (cached) {
		return cached as Record<string, GrpcUnaryMethod>;
	}

	const channel = createChannel(
		target,
		undefined,
		authority ? { 'grpc.default_authority': authority } : undefined
	);
	const client = createClient(serviceDefinitions[service], channel);
	clients.set(cacheKey, client);

	return client as Record<string, GrpcUnaryMethod>;
}

export function getBootstrapClient(): BootstrapServiceClient {
	return getGrpcClient('bootstrap') as unknown as BootstrapServiceClient;
}

export function getChatClient(): ChatServiceClient {
	return getGrpcClient('chat') as unknown as ChatServiceClient;
}

export function getFriendshipClient(): FriendshipServiceClient {
	return getGrpcClient('friendship') as unknown as FriendshipServiceClient;
}

export function getIdentityClient(): IdentityServiceClient {
	return getGrpcClient('identity') as unknown as IdentityServiceClient;
}

export function getRealtimeClient(): RealtimeServiceClient {
	return getGrpcClient('realtime') as unknown as RealtimeServiceClient;
}

export function getWorkspaceClient(): WorkspaceServiceClient {
	return getGrpcClient('workspace') as unknown as WorkspaceServiceClient;
}

export function metadataFromHeaders(headers: Headers): Metadata {
	const metadata = Metadata();
	const authorizationHeaderName = env.AUTHORIZATION_HEADER_NAME ?? 'authorization';
	const forwardedHeaders = [
		authorizationHeaderName,
		'x-request-id',
		'x-correlation-id',
		'x-relay-actor-user-id'
	];

	for (const header of forwardedHeaders) {
		const value = headers.get(header);

		if (value) {
			metadata.set(header, value);
		}
	}

	return metadata;
}

export function metadataFromRequest(headers: Headers, cookies: Cookies): Metadata {
	const metadata = metadataFromHeaders(headers);
	const authorizationHeaderName = env.AUTHORIZATION_HEADER_NAME ?? 'authorization';
	const accessTokenCookieName = env.ACCESS_TOKEN_COOKIE_NAME ?? 'access_token';
	const bearerToken =
		headers.get(authorizationHeaderName) ?? bearerFromCookie(cookies.get(accessTokenCookieName));

	if (bearerToken) {
		metadata.set('authorization', bearerToken);
	}

	return metadata;
}

function bearerFromCookie(accessToken: string | undefined): string | undefined {
	return accessToken ? `Bearer ${accessToken}` : undefined;
}

export function reviveGrpcRequest(value: unknown): unknown {
	if (Array.isArray(value)) {
		return value.map(reviveGrpcRequest);
	}

	if (value && typeof value === 'object') {
		return Object.fromEntries(
			Object.entries(value).map(([key, entry]) => [
				key,
				typeof entry === 'string' && key.endsWith('At') ? new Date(entry) : reviveGrpcRequest(entry)
			])
		);
	}

	return value;
}

export function grpcErrorToHttp(cause: unknown): { status: number; message: string } {
	if (isRedirect(cause)) {
		throw cause;
	}

	if (isHttpError(cause)) {
		return { status: cause.status, message: cause.body.message };
	}

	const grpcError = cause as Partial<ClientError>;

	if (typeof grpcError.code !== 'number') {
		return {
			status: 500,
			message: cause instanceof Error ? cause.message : 'Unexpected gRPC client error'
		};
	}

	return {
		status: grpcStatusToHttpStatus(grpcError.code),
		message: grpcError.details ?? 'gRPC request failed'
	};
}

type GrpcUnaryMethod = (request: unknown, options?: { metadata?: Metadata }) => Promise<unknown>;

function grpcStatusToHttpStatus(status: Status): number {
	switch (status) {
		case Status.CANCELLED:
			return 499;
		case Status.INVALID_ARGUMENT:
			return 400;
		case Status.DEADLINE_EXCEEDED:
			return 504;
		case Status.NOT_FOUND:
			return 404;
		case Status.ALREADY_EXISTS:
			return 409;
		case Status.PERMISSION_DENIED:
			return 403;
		case Status.RESOURCE_EXHAUSTED:
			return 429;
		case Status.FAILED_PRECONDITION:
			return 412;
		case Status.ABORTED:
			return 409;
		case Status.OUT_OF_RANGE:
			return 400;
		case Status.UNIMPLEMENTED:
			return 501;
		case Status.UNAVAILABLE:
			return 503;
		case Status.UNAUTHENTICATED:
			return 401;
		default:
			return 500;
	}
}
