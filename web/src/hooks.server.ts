import { json, redirect } from '@sveltejs/kit';

import { accessTokenCookieName } from '$lib/server/auth-cookies';

import type { Handle } from '@sveltejs/kit';

const publicPrefixes = ['/auth/', '/_app/', '/favicon'];
const publicFiles = /\.(?:css|js|ico|png|jpg|jpeg|svg|webp|avif|gif|woff2?)$/i;

export const handle: Handle = async ({ event, resolve }) => {
	if (isPublicPath(event.url.pathname) || event.cookies.get(accessTokenCookieName())) {
		return resolve(event);
	}

	if (event.url.pathname.startsWith('/api/')) {
		return json({ message: 'Authentication required' }, { status: 401 });
	}

	const redirectTo = `${event.url.pathname}${event.url.search}`;
	redirect(303, `/auth/login?redirectTo=${encodeURIComponent(redirectTo)}`);
};

function isPublicPath(pathname: string): boolean {
	return publicPrefixes.some((prefix) => pathname.startsWith(prefix)) || publicFiles.test(pathname);
}
