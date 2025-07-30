"use client"

import { useSession, getSession } from "next-auth/react"
import { useState } from "react"

export function ProtectedApiTest() {
  const { data: session, status } = useSession()
  const [response, setResponse] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const callProtectedApi = async () => {
    setLoading(true)
    setError(null)
    setResponse(null)
    
    try {
      // Force session refresh to ensure we have a fresh JWT token
      const session = await getSession()
      
      const res = await fetch("http://localhost:8080/api/protected", {
        headers: {
          Authorization: `Bearer ${session?.accessToken}`,
        },
      })
      const text = await res.text()
      if (!res.ok) {
        throw new Error(text)
      }
      setResponse(text)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  if (status === "loading") {
    return <div>Loading session...</div>
  }

  if (!session) {
    return <div>You must be signed in to test the protected API.</div>
  }

  return (
    <div className="space-y-4">
      <button
        onClick={callProtectedApi}
        className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
        disabled={loading}
      >
        {loading ? "Calling API..." : "Call Protected API"}
      </button>
      {response && (
        <div className="bg-green-50 p-4 rounded-lg border border-green-200 shadow-sm">
          <h3 className="font-semibold text-green-800 mb-3 flex items-center">
            <svg className="w-5 h-5 mr-2" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
            </svg>
            API Response
          </h3>
          <div className="text-green-900 whitespace-pre-wrap break-words font-mono text-sm bg-white p-3 rounded border border-green-100">
            {response}
          </div>
        </div>
      )}
      {error && (
        <div className="bg-red-50 p-4 rounded-lg border border-red-200 shadow-sm">
          <h3 className="font-semibold text-red-800 mb-3 flex items-center">
            <svg className="w-5 h-5 mr-2" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
            </svg>
            Error
          </h3>
          <div className="text-red-900 bg-white p-3 rounded border border-red-100">
            {error}
          </div>
        </div>
      )}
    </div>
  )
} 