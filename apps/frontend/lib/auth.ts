import NextAuth, { type NextAuthConfig } from 'next-auth';
import GoogleProvider from 'next-auth/providers/google';
import { SignJWT } from 'jose';
import type { JWT } from 'next-auth/jwt';
import type { Account, Session, User } from 'next-auth';
import type { AdapterUser } from 'next-auth/adapters';
const secret = new TextEncoder().encode(process.env.AUTH_SECRET);
const TOKEN_LIFESPAN_SECONDS = 60 * 60; // 1 hour

async function signToken(payload: { sub: string; email?: string }) {
  const now = Math.floor(Date.now() / 1000);
  const exp = now + TOKEN_LIFESPAN_SECONDS;

  return new SignJWT(payload)
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt(now)
    .setExpirationTime(exp)
    .sign(secret);
}

type AugmentedJWT = JWT & {
  accessToken?: string;
  accessTokenCreatedAt?: number;
};

export const config: NextAuthConfig = {
  providers: [
    GoogleProvider({
      clientId: process.env.GOOGLE_CLIENT_ID!,
      clientSecret: process.env.GOOGLE_CLIENT_SECRET!,
    }),
  ],
  session: { strategy: 'jwt' },
  callbacks: {
    async jwt({
      token,
      user,
      account: _account,
    }: {
      token: JWT;
      user?: User | AdapterUser;
      account?: Account | null;
      profile?: unknown;
      trigger?: 'signIn' | 'signUp' | 'update';
      isNewUser?: boolean;
      session?: unknown;
    }): Promise<JWT> {
      // Persist id/email from user when it is present (e.g., on sign-in)
      if (user) {
        const maybeId =
          (user as AdapterUser).id ?? (user as unknown as { id?: string | null })?.id ?? token.sub;
        if (typeof maybeId === 'string') token.sub = maybeId;

        if ('email' in user) {
          const email = (user as { email?: string | null }).email;
          token.email = email ?? undefined;
        }
      }

      const aug = token as AugmentedJWT;
      const now = Math.floor(Date.now() / 1000);
      const createdAt = aug.accessTokenCreatedAt ?? 0;
      const isExpired = now - createdAt > TOKEN_LIFESPAN_SECONDS;

      if (!aug.accessToken || isExpired) {
        aug.accessToken = await signToken({
          sub: token.sub ?? '',
          email: token.email ?? undefined,
        });
        aug.accessTokenCreatedAt = now;
      }

      return token;
    },

    async session({
      session,
      token,
    }: {
      session: Session;
      token: JWT;
      user: User | AdapterUser | undefined;
    }): Promise<Session> {
      (session as Session & { accessToken?: string }).accessToken = (
        token as AugmentedJWT
      ).accessToken;
      return session;
    },
  },
};

export const { auth, signIn, signOut, handlers } = NextAuth(config);
