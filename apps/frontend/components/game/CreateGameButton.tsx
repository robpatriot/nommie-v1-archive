'use client';

import { useSession, getSession } from 'next-auth/react';
import { useState, useCallback } from 'react';
import { BACKEND_URL } from '@/lib/config';

interface GamePlayer {
  id: string;
  game_id: string;
  user_id: string;
  turn_order: number | null;
  is_ready: boolean;
}

interface Game {
  id: string;
  state: string;
  created_at: string;
  updated_at: string;
}

interface CreateGameResponse {
  game: Game;
  game_players: GamePlayer[];
}

export function CreateGameButton() {
  const { data: session, status } = useSession();
  const [response, setResponse] = useState<CreateGameResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const createGame = useCallback(async () => {
    setLoading(true);
    setError(null);
    setResponse(null);

    try {
      // Force a fresh session fetch so our v5 JWT callback can refresh the token if expired
      const freshSession = await getSession();

      const accessToken = freshSession?.accessToken;
      if (!accessToken) {
        throw new Error('No access token available (please sign in again).');
      }

      const res = await fetch(`${BACKEND_URL}/api/create_game`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${accessToken}`,
          'Content-Type': 'application/json',
          Accept: 'application/json',
        },
      });

      if (!res.ok) {
        const errorText = await res.text().catch(() => '');
        throw new Error(`HTTP ${res.status}${errorText ? `: ${errorText}` : ''}`.trim());
      }

      const data: CreateGameResponse = await res.json();
      setResponse(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  if (status === 'loading') {
    return <div>Loading session...</div>;
  }

  if (!session) {
    return <div>You must be signed in to create a game.</div>;
  }

  const tokenPresent = Boolean(session.accessToken);

  return (
    <div className="space-y-4">
      <button
        onClick={createGame}
        className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
        disabled={loading || !tokenPresent}
        title={!tokenPresent ? 'No access token available' : undefined}
      >
        {loading ? 'Creating Game...' : 'Create Game'}
      </button>

      {response && (
        <div className="bg-green-50 p-4 rounded-lg border border-green-200 shadow-sm">
          <h3 className="font-semibold text-green-800 mb-3 flex items-center">
            <svg
              className="w-5 h-5 mr-2"
              fill="currentColor"
              viewBox="0 0 20 20"
              aria-hidden="true"
            >
              <path
                fillRule="evenodd"
                d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                clipRule="evenodd"
              />
            </svg>
            Game Created Successfully!
          </h3>
          <div className="space-y-3">
            <div className="bg-white p-3 rounded border border-green-100">
              <h4 className="font-medium text-green-900 mb-2">Game Details:</h4>
              <div className="text-sm text-green-800 space-y-1">
                <div>
                  <strong>Game ID:</strong> {response.game.id}
                </div>
                <div>
                  <strong>State:</strong> {response.game.state}
                </div>
                <div>
                  <strong>Created:</strong> {new Date(response.game.created_at).toLocaleString()}
                </div>
                <div>
                  <strong>Updated:</strong> {new Date(response.game.updated_at).toLocaleString()}
                </div>
              </div>
            </div>

            <div className="bg-white p-3 rounded border border-green-100">
              <h4 className="font-medium text-green-900 mb-2">
                Players ({response.game_players.length}):
              </h4>
              <div className="text-sm text-green-800 space-y-1">
                {response.game_players.map((player, index) => {
                  const displayName =
                    session.user?.email || `User ${player.user_id.slice(0, 8)}...`;
                  return (
                    <div key={player.id} className="border-l-2 border-green-200 pl-2">
                      <div>
                        <strong>Player {index + 1}:</strong> {displayName}
                      </div>
                      <div>
                        <strong>Ready:</strong> {player.is_ready ? 'Yes' : 'No'}
                      </div>
                      <div>
                        <strong>Turn Order:</strong> {player.turn_order ?? 'Not set'}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}

      {error && (
        <div className="bg-red-50 p-4 rounded-lg border border-red-200 shadow-sm">
          <h3 className="font-semibold text-red-800 mb-3 flex items-center">
            <svg
              className="w-5 h-5 mr-2"
              fill="currentColor"
              viewBox="0 0 20 20"
              aria-hidden="true"
            >
              <path
                fillRule="evenodd"
                d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
                clipRule="evenodd"
              />
            </svg>
            Error Creating Game
          </h3>
          <div className="text-red-900 bg-white p-3 rounded border border-red-100">{error}</div>
        </div>
      )}
    </div>
  );
}
