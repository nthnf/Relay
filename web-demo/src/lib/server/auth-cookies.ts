import { dev } from '$app/environment';
import { env } from '$env/dynamic/private';

import type { Cookies } from '@sveltejs/kit';
import type { TokenPairResponse } from '../../generated/identity';

const cookieDefaults = {
	httpOnly: true,
	sameSite: 'lax' as const,
	secure: !dev,
	path: '/'
};

export function accessTokenCookieName(): string {
	return env.ACCESS_TOKEN_COOKIE_NAME ?? 'access_token';
}

export function refreshTokenCookieName(): string {
	return env.REFRESH_TOKEN_COOKIE_NAME ?? 'refresh_token';
}

export function sessionIdCookieName(): string {
	return env.SESSION_ID_COOKIE_NAME ?? 'session_id';
}

export function setAuthCookies(cookies: Cookies, tokenPair: TokenPairResponse): void {
	cookies.set(accessTokenCookieName(), tokenPair.accessToken, {
		...cookieDefaults,
		maxAge: maxAgeSeconds(tokenPair.accessTokenExpiresAt)
	});
	cookies.set(refreshTokenCookieName(), tokenPair.refreshToken, {
		...cookieDefaults,
		maxAge: maxAgeSeconds(tokenPair.refreshTokenExpiresAt)
	});
	cookies.set(sessionIdCookieName(), tokenPair.sessionId, {
		...cookieDefaults,
		maxAge: maxAgeSeconds(tokenPair.refreshTokenExpiresAt)
	});
}

export function clearAuthCookies(cookies: Cookies): void {
	cookies.delete(accessTokenCookieName(), { path: '/' });
	cookies.delete(refreshTokenCookieName(), { path: '/' });
	cookies.delete(sessionIdCookieName(), { path: '/' });
}

export function tokenPairJson(tokenPair: TokenPairResponse): Record<string, unknown> {
	return {
		userId: tokenPair.userId,
		sessionId: tokenPair.sessionId,
		accessTokenExpiresAt: tokenPair.accessTokenExpiresAt,
		refreshTokenExpiresAt: tokenPair.refreshTokenExpiresAt,
		emailVerified: tokenPair.emailVerified,
		profile: tokenPair.profile
	};
}

function maxAgeSeconds(expiresAt: Date | undefined): number | undefined {
	if (!expiresAt) {
		return undefined;
	}

	return Math.max(0, Math.floor((expiresAt.getTime() - Date.now()) / 1000));
}
