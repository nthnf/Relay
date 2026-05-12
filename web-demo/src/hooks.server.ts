import { json, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getIdentityClient, metadataFromRequest } from '$lib/grpc/client.server';
import {
	accessTokenCookieName,
	clearAuthCookies,
	refreshTokenCookieName,
	setAuthCookies
} from '$lib/server/auth-cookies';
import { encodeRouteId } from '$lib/server/route-ids';

import type { Handle } from '@sveltejs/kit';

const publicPrefixes = ['/auth/', '/verify-email', '/_app/', '/favicon'];
const publicFiles = /\.(?:css|js|ico|png|jpg|jpeg|svg|webp|avif|gif|woff2?)$/i;

export const handle: Handle = async ({ event, resolve }) => {
	let authenticated = Boolean(event.cookies.get(accessTokenCookieName()));

	if (!authenticated && shouldAttemptRefresh(event.url.pathname)) {
		authenticated = await refreshSession(event);
	}

	if (authenticated && isAuthPage(event.url.pathname)) {
		redirect(303, await defaultAuthenticatedRoute(event));
	}

	if (isPublicPath(event.url.pathname) || authenticated) {
		return resolve(event);
	}

	if (event.url.pathname.startsWith('/api/')) {
		return json({ message: 'Authentication required' }, { status: 401 });
	}

	const redirectTo = `${event.url.pathname}${event.url.search}`;
	redirect(303, `/auth/login?redirectTo=${encodeURIComponent(redirectTo)}`);
};

function isPublicPath(pathname: string): boolean {
	return pathname === '/' || publicPrefixes.some((prefix) => pathname.startsWith(prefix)) || publicFiles.test(pathname);
}

function isAuthPage(pathname: string): boolean {
	return pathname.startsWith('/auth/') && !pathname.startsWith('/auth/logout') && !pathname.startsWith('/auth/refresh');
}

function shouldAttemptRefresh(pathname: string): boolean {
	if (pathname === '/' || pathname.startsWith('/auth/refresh') || publicFiles.test(pathname)) {
		return false;
	}

	return true;
}

async function refreshSession(event: Parameters<Handle>[0]['event']): Promise<boolean> {
	const refreshToken = event.cookies.get(refreshTokenCookieName());

	if (!refreshToken) {
		return false;
	}

	try {
		const tokenPair = await getIdentityClient().refreshSession({ refreshToken });
		setAuthCookies(event.cookies, tokenPair);
		return true;
	} catch {
		clearAuthCookies(event.cookies);
		return false;
	}
}

async function defaultAuthenticatedRoute(event: Parameters<Handle>[0]['event']): Promise<string> {
	try {
		const metadata = metadataFromRequest(event.request.headers, event.cookies);
		const app = await getBootstrapClient().getAppBootstrap({}, { metadata });
		const workspace = app.workspaces[0];

		if (!workspace) {
			return '/profile';
		}

		const workspaceBootstrap = await getBootstrapClient().getWorkspaceBootstrap(
			{ workspaceId: workspace.workspaceId },
			{ metadata }
		);
		const channel = workspaceBootstrap.channels[0];

		if (!channel) {
			return `/workspace/${encodeRouteId(workspace.workspaceId)}`;
		}

		return `/workspace/${encodeRouteId(workspace.workspaceId)}/${encodeRouteId(channel.channelId)}`;
	} catch {
		return '/profile';
	}
}
