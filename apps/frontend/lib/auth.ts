import { NextAuthOptions } from "next-auth"
import GoogleProvider from "next-auth/providers/google"
import { SignJWT } from "jose"

const secret = new TextEncoder().encode(process.env.AUTH_SECRET)
const TOKEN_LIFESPAN_SECONDS = 60 * 60 // 1 hour

async function signToken(userToken: Record<string, any>) {
  const now = Math.floor(Date.now() / 1000)
  const exp = now + TOKEN_LIFESPAN_SECONDS
  
  // Create a clean payload with only user claims
  // Let jose handle iat and exp automatically
  const payload = {
    sub: userToken.sub,
    email: userToken.email
  }
  

  
  const token = await new SignJWT(payload)
    .setProtectedHeader({ alg: "HS256" })
    .setIssuedAt(now)
    .setExpirationTime(exp)
    .sign(secret)
  

  return token
}

export const authOptions: NextAuthOptions = {
  providers: [
    GoogleProvider({
      clientId: process.env.GOOGLE_CLIENT_ID!,
      clientSecret: process.env.GOOGLE_CLIENT_SECRET!,
    }),
  ],
  session: {
    strategy: "jwt",
  },
  jwt: {
    secret: process.env.AUTH_SECRET,
  },
  callbacks: {
    async jwt({ token, account, user }) {
      if (account && user) {
        token.sub = user.id
        token.email = user.email
      }

      const now = Math.floor(Date.now() / 1000)
      const createdAt = (token.accessTokenCreatedAt as number) ?? 0
      const isExpired = now - createdAt > TOKEN_LIFESPAN_SECONDS

      if (!token.accessToken || isExpired) {
        token.accessToken = await signToken(token)
        token.accessTokenCreatedAt = now
      }

      return token
    },
    async session({ session, token }) {
      session.accessToken = token.accessToken
      return session
    }
  }
}
