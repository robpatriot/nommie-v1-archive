'use client';

import { useState, useCallback } from 'react';
import { useSession } from 'next-auth/react';
import { useRouter } from 'next/navigation';
import { CreateGameButton } from '@/components/game/CreateGameButton';
import { BACKEND_URL } from '@/lib/config';
import { usePolling } from '@/hooks/usePolling';

interface Game {
  id: string;
  state: string;
  player_count: number;
  max_players?: number;
  is_player_in_game?: boolean;
}

interface JoinGameResponse {
  success: boolean;
  message?: string;
  game_id?: string;
}

export default function LobbyPage() {
  const { data: session, status } = useSession();
  const router = useRouter();
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [joiningGame, setJoiningGame] = useState<string | null>(null);
  const [createGameLoading, setCreateGameLoading] = useState(false);

  const fetchGames = useCallback(async () => {
    setLoading(true);
    setError(null);
    
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/games`, {
        headers: {
          'Authorization': `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }
      
      const data = await response.json();
      setGames(data.games || data); // Handle both array and object with games property
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch games');
    } finally {
      setLoading(false);
    }
  }, []);

  // Use the polling hook
  const { isPolling } = usePolling({
    enabled: status === 'authenticated',
    interval: 5000, // 5 seconds
    callback: fetchGames,
    immediate: true,
  });

  const joinGame = async (gameId: string) => {
    setJoiningGame(gameId);
    setError(null);
    
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/join_game?game_id=${gameId}`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }
      
      const data: JoinGameResponse = await response.json();
      
      if (data.success) {
        // Redirect to the game page
        router.push(`/game/${gameId}`);
      } else {
        throw new Error(data.message || 'Failed to join game');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setJoiningGame(null);
    }
  };

  const createGameAndJoin = async () => {
    setCreateGameLoading(true);
    setError(null);
    
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/create_game`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }
      
      const data = await response.json();
      
      // Redirect to the newly created game
      router.push(`/game/${data.game.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreateGameLoading(false);
    }
  };

  const navigateToGame = (gameId: string) => {
    router.push(`/game/${gameId}`);
  };

  // Show loading state
  if (status === 'loading') {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto"></div>
          <p className="mt-4 text-gray-600">Loading...</p>
        </div>
      </div>
    );
  }

  // Show login prompt if not authenticated
  if (status === 'unauthenticated') {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">Please Sign In</h1>
          <p className="text-gray-600 mb-4">You need to be signed in to access the lobby.</p>
          <button
            onClick={() => router.push('/')}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
          >
            Go to Home
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900">
      <div className="max-w-6xl mx-auto px-4 py-8">
        {/* Header */}
        <div className="mb-8">
          <div className="flex items-center justify-between mb-4">
            <h1 className="text-3xl font-bold text-gray-900 dark:text-white">
              Game Lobby
            </h1>
            <a
              href="/"
              className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700 font-medium"
            >
              Back to Home
            </a>
          </div>
          <p className="text-gray-600 dark:text-gray-400">
            Welcome back, {session?.user?.email}
          </p>
        </div>

        {/* Create Game Section */}
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6 mb-8">
          <h2 className="text-xl font-semibold mb-4 text-gray-900 dark:text-white">
            Create New Game
          </h2>
          <button
            onClick={createGameAndJoin}
            disabled={createGameLoading}
            className="px-6 py-3 bg-green-600 text-white rounded-lg hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
          >
            {createGameLoading ? (
              <div className="flex items-center">
                <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                Creating Game...
              </div>
            ) : (
              'Create New Game'
            )}
          </button>
        </div>

        {/* Games List */}
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
          <div className="flex items-center justify-between mb-6">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
              Available Games
            </h2>
            <div className="flex items-center space-x-3">
              {isPolling && (
                <div className="flex items-center text-sm text-gray-600 dark:text-gray-400">
                  <div className="animate-spin rounded-full h-3 w-3 border-b-2 border-blue-600 mr-2"></div>
                  Auto-refreshing every 5 seconds
                </div>
              )}
              <button
                onClick={fetchGames}
                disabled={loading}
                className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700 disabled:opacity-50"
              >
                {loading ? 'Refreshing...' : 'Refresh'}
              </button>
            </div>
          </div>

          {error && (
            <div className="mb-6 bg-red-50 border border-red-200 rounded-lg p-4">
              <div className="flex items-center">
                <svg className="w-5 h-5 text-red-400 mr-2" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
                </svg>
                <span className="text-red-800 font-medium">Error</span>
              </div>
              <p className="text-red-700 mt-1">{error}</p>
            </div>
          )}

          {loading ? (
            <div className="text-center py-8">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
              <p className="mt-2 text-gray-600 dark:text-gray-400">Loading games...</p>
            </div>
          ) : games.length === 0 ? (
            <div className="text-center py-8">
              <div className="text-gray-400 dark:text-gray-500 mb-2">
                <svg className="w-12 h-12 mx-auto" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                </svg>
              </div>
              <p className="text-gray-600 dark:text-gray-400">No games available</p>
              <p className="text-sm text-gray-500 dark:text-gray-500 mt-1">
                Create a new game to get started!
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              {games.map((game) => {
                const maxPlayers = game.max_players || 4;
                const canJoin = !game.is_player_in_game && game.player_count < maxPlayers;
                
                return (
                  <div
                    key={game.id}
                    className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors cursor-pointer"
                    onClick={() => navigateToGame(game.id)}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <div className="flex items-center space-x-3">
                          <div className="flex items-center space-x-2">
                            <span className="text-sm font-medium text-gray-900 dark:text-white">
                              Game {game.id.slice(0, 8)}...
                            </span>
                            <span className={`px-2 py-1 text-xs rounded-full ${
                              game.state === 'waiting' 
                                ? 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200'
                                : game.state === 'started'
                                ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200'
                                : 'bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-200'
                            }`}>
                              {game.state.charAt(0).toUpperCase() + game.state.slice(1)}
                            </span>
                          </div>
                        </div>
                        <div className="mt-1 text-sm text-gray-600 dark:text-gray-400">
                          Players: {game.player_count}/{maxPlayers}
                        </div>
                      </div>
                      
                      <div className="flex items-center space-x-2">
                        {game.is_player_in_game ? (
                          <span className="px-3 py-1 text-sm bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200 rounded-full">
                            You're in this game
                          </span>
                        ) : canJoin ? (
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              joinGame(game.id);
                            }}
                            disabled={joiningGame === game.id}
                            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium"
                          >
                            {joiningGame === game.id ? (
                              <div className="flex items-center">
                                <div className="animate-spin rounded-full h-3 w-3 border-b-2 border-white mr-1"></div>
                                Joining...
                              </div>
                            ) : (
                              'Join'
                            )}
                          </button>
                        ) : (
                          <span className="px-3 py-1 text-sm bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300 rounded-full">
                            Full
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
} 