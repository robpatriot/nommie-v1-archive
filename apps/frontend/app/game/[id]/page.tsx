'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { useRouter } from 'next/navigation';
import { BACKEND_URL } from '@/lib/config';

interface Player {
  id: string;
  user_id: string;
  turn_order: number | null;
  is_ready: boolean;
  is_ai: boolean;
  user: {
    id: string;
    email: string;
    name: string | null;
  };
}

interface Game {
  id: string;
  state: string;
  created_at: string;
  updated_at: string;
  started_at?: string;
}

interface GameData {
  game: Game;
  players: Player[];
  player_count: number;
  max_players: number;
}

interface GameState {
  game: {
    id: string;
    state: string;
    current_turn: number | null;
    created_at: string;
    updated_at: string;
    started_at?: string;
  };
  players: Player[];
  player_count: number;
  max_players: number;
}

export default function GamePage() {
  const params = useParams();
  const { data: session, status } = useSession();
  const router = useRouter();
  const gameId = params.id as string;

  const [gameData, setGameData] = useState<GameData | null>(null);
  const [gameState, setGameState] = useState<GameState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [pollingInterval, setPollingInterval] = useState<NodeJS.Timeout | null>(null);
  const [gameStatePollingInterval, setGameStatePollingInterval] = useState<NodeJS.Timeout | null>(null);
  const [markingReady, setMarkingReady] = useState(false);
  const [addingAI, setAddingAI] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const fetchGameData = useCallback(async (isManualRefresh = false) => {
    if (isManualRefresh) {
      setRefreshing(true);
    } else {
      setLoading(true);
    }
    setError(null);
    
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/state`, {
        headers: {
          'Authorization': `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }
      
      const data: GameState = await response.json();
      // Convert GameState to GameData format for backward compatibility
      const gameData: GameData = {
        game: {
          id: data.game.id,
          state: data.game.state,
          created_at: data.game.created_at,
          updated_at: data.game.updated_at,
          started_at: data.game.started_at,
        },
        players: data.players,
        player_count: data.player_count,
        max_players: data.max_players,
      };
      setGameData(gameData);
      setGameState(data);
      
      // Stop polling if game has started
      if (data.game.state.toLowerCase() === 'started') {
        setPollingInterval(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch game data');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [gameId]);

  const fetchGameState = useCallback(async () => {
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/state`, {
        headers: {
          'Authorization': `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }
      
      const data: GameState = await response.json();
      setGameState(data);
    } catch (err) {
      console.error('Failed to fetch game state:', err);
    }
  }, [gameId]);

  // Start polling when component mounts
  useEffect(() => {
    if (status === 'authenticated' && gameId) {
      fetchGameData();
      
      // Start polling every 3 seconds
      const interval = setInterval(() => {
        fetchGameData();
      }, 3000);
      
      setPollingInterval(interval);
      
      // Cleanup on unmount
      return () => {
        clearInterval(interval);
      };
    } else if (status === 'unauthenticated') {
      setLoading(false);
    }
  }, [status, gameId, fetchGameData]);

  // Start game state polling when game starts
  useEffect(() => {
    if (gameData?.game.state.toLowerCase() === 'started' && !gameStatePollingInterval) {
      // Fetch initial game state
      fetchGameState();
      
      // Start polling game state every 2.5 seconds
      const interval = setInterval(() => {
        fetchGameState();
      }, 2500);
      
      setGameStatePollingInterval(interval);
      
      // Cleanup on unmount
      return () => {
        clearInterval(interval);
      };
    }
  }, [gameData?.game.state, gameStatePollingInterval, fetchGameState]);

  // Stop polling when game starts
  useEffect(() => {
    if (gameData?.game.state.toLowerCase() === 'started' && pollingInterval) {
      clearInterval(pollingInterval);
      setPollingInterval(null);
    }
  }, [gameData?.game.state, pollingInterval]);

  // Cleanup intervals on unmount
  useEffect(() => {
    return () => {
      if (pollingInterval) {
        clearInterval(pollingInterval);
      }
      if (gameStatePollingInterval) {
        clearInterval(gameStatePollingInterval);
      }
    };
  }, [pollingInterval, gameStatePollingInterval]);

  const handleManualRefresh = () => {
    fetchGameData(true);
  };

  const handleMarkReady = async () => {
    setMarkingReady(true);
    setError(null);
    setSuccessMessage(null);
    
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/ready`, {
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
      
      if (data.success) {
        // Update local state immediately
        if (gameData) {
          setGameData({
            ...gameData,
            players: gameData.players.map(player => 
              player.user.email === session.user?.email 
                ? { ...player, is_ready: true }
                : player
            ),
            // Update game state if the game started
            game: data.game_started ? {
              ...gameData.game,
              state: 'Started',
              started_at: new Date().toISOString()
            } : gameData.game
          });
        }
        
        // Show success message
        if (data.game_started) {
          setSuccessMessage('Game started! All players are ready.');
        } else {
          setSuccessMessage('Marked as ready!');
        }
      } else {
        throw new Error(data.message || 'Failed to mark as ready');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to mark as ready');
    } finally {
      setMarkingReady(false);
    }
  };

  const handleAddAI = async () => {
    setAddingAI(true);
    setError(null);
    setSuccessMessage(null);
    
    try {
      const session = await import('next-auth/react').then(m => m.getSession());
      
      if (!session?.accessToken) {
        throw new Error('No access token available');
      }
      
      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/add_ai`, {
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
      
      if (data.success) {
        // Refresh game data to show the new AI player
        await fetchGameData(true);
        
        // Show success message
        if (data.game_started) {
          setSuccessMessage('AI player added and game started!');
        } else {
          setSuccessMessage('AI player added successfully!');
        }
      } else {
        throw new Error(data.message || 'Failed to add AI player');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add AI player');
    } finally {
      setAddingAI(false);
    }
  };

  const getGameStateColor = (state: string) => {
    switch (state) {
      case 'waiting':
        return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200';
      case 'started':
        return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200';
      case 'finished':
        return 'bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-200';
      default:
        return 'bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-200';
    }
  };

  const getGameStateText = (state: string) => {
    return state.charAt(0).toUpperCase() + state.slice(1);
  };

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

  if (status === 'unauthenticated') {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">Please Sign In</h1>
          <p className="text-gray-600 mb-4">You need to be signed in to access the game.</p>
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
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center space-x-4">
              <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                Game {gameId.slice(0, 8)}...
              </h1>
              {gameData && (
                <span className={`px-3 py-1 text-sm rounded-full font-medium ${getGameStateColor(gameData.game.state)}`}>
                  {getGameStateText(gameData.game.state)}
                </span>
              )}
            </div>
            <div className="flex items-center space-x-3">
              <button
                onClick={handleManualRefresh}
                disabled={refreshing}
                className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {refreshing ? (
                  <div className="flex items-center">
                    <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                    Refreshing...
                  </div>
                ) : (
                  'Refresh'
                )}
              </button>
              <button
                onClick={() => router.push('/lobby')}
                className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700"
              >
                Back to Lobby
              </button>
            </div>
          </div>
          
          {gameData && (
            <div className="text-sm text-gray-600 dark:text-gray-400">
              Players: {gameData.player_count}/{gameData.max_players}
              {pollingInterval && (
                <span className="ml-4 text-green-600 dark:text-green-400">
                  • Auto-refreshing every 3 seconds
                </span>
              )}
              {!pollingInterval && gameData.game.state === 'started' && (
                <span className="ml-4 text-orange-600 dark:text-orange-400">
                  • Game started - polling stopped
                </span>
              )}
            </div>
          )}
        </div>

        {/* Error Display */}
        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-4 mb-6">
            <div className="flex items-center">
              <svg className="w-5 h-5 text-red-400 mr-2" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1-1z" clipRule="evenodd" />
              </svg>
              <span className="text-red-800 font-medium">Error</span>
            </div>
            <p className="text-red-700 mt-1">{error}</p>
          </div>
        )}

        {/* Success Display */}
        {successMessage && (
          <div className="bg-green-50 border border-green-200 rounded-lg p-4 mb-6">
            <div className="flex items-center">
              <svg className="w-5 h-5 text-green-400 mr-2" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
              </svg>
              <span className="text-green-800 font-medium">Success</span>
            </div>
            <p className="text-green-700 mt-1">{successMessage}</p>
          </div>
        )}

        {/* Loading State */}
        {loading && !gameData && (
          <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
            <div className="text-center py-8">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
              <p className="mt-2 text-gray-600 dark:text-gray-400">Loading game data...</p>
            </div>
          </div>
        )}

        {/* Game Data */}
        {gameData && (
          <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
            
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
                Players
              </h2>
              
              {/* Ready Button */}
              {gameData.game.state.toLowerCase() === 'waiting' && (
                <div className="flex space-x-2">
                  <button
                    onClick={handleMarkReady}
                    disabled={markingReady || gameData.players.find(p => p.user.email === session?.user?.email)?.is_ready}
                    className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
                  >
                    {markingReady ? (
                      <div className="flex items-center">
                        <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                        Marking Ready...
                      </div>
                    ) : gameData.players.find(p => p.user.email === session?.user?.email)?.is_ready ? (
                      'Ready ✓'
                    ) : (
                      'Mark Ready'
                    )}
                  </button>
                  <button
                    onClick={handleAddAI}
                    disabled={addingAI || gameData.player_count >= gameData.max_players}
                    className="px-4 py-2 bg-purple-600 text-white rounded hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
                  >
                    {addingAI ? (
                      <div className="flex items-center">
                        <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                        Adding AI...
                      </div>
                    ) : (
                      'Add AI Player'
                    )}
                  </button>
                </div>
              )}
            </div>
            
            {gameData.players.length === 0 ? (
              <div className="text-center py-8">
                <div className="text-gray-400 dark:text-gray-500 mb-2">
                  <svg className="w-12 h-12 mx-auto" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197m13.5-9a2.5 2.5 0 11-5 0 2.5 2.5 0 015 0z" />
                  </svg>
                </div>
                <p className="text-gray-600 dark:text-gray-400">No players in game</p>
              </div>
            ) : (
              <div className="space-y-3">
                {(gameState?.players || gameData.players).map((player) => {
                  const isCurrentUser = player.user.email === session?.user?.email;
                  const isCurrentTurn = gameState && gameState.game.current_turn !== null && 
                    player.turn_order === gameState.game.current_turn;
                  
                  return (
                    <div
                      key={player.id}
                      className={`flex items-center justify-between p-4 border rounded-lg ${
                        isCurrentUser 
                          ? 'border-blue-300 bg-blue-50 dark:border-blue-600 dark:bg-blue-900/20' 
                          : isCurrentTurn
                          ? 'border-orange-300 bg-orange-50 dark:border-orange-600 dark:bg-orange-900/20'
                          : 'border-gray-200 dark:border-gray-700'
                      }`}
                    >
                      <div className="flex items-center space-x-3">
                        <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
                          player.is_ai 
                            ? 'bg-purple-100 dark:bg-purple-900' 
                            : 'bg-blue-100 dark:bg-blue-900'
                        }`}>
                          <span className={`font-medium ${
                            player.is_ai 
                              ? 'text-purple-600 dark:text-purple-400' 
                              : 'text-blue-600 dark:text-blue-400'
                          }`}>
                            {player.is_ai ? '🤖' : (player.user.name || player.user.email).charAt(0).toUpperCase()}
                          </span>
                        </div>
                        <div>
                          <p className={`font-medium text-gray-900 dark:text-white ${
                            isCurrentTurn ? 'font-bold' : ''
                          }`}>
                            {player.user.name || player.user.email}
                            {isCurrentTurn && (
                              <span className="ml-2 inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200">
                                Current Turn
                              </span>
                            )}
                          </p>
                          <p className="text-sm text-gray-500 dark:text-gray-400">
                            {player.user.email}
                          </p>
                        </div>
                      </div>
                      
                      <div className="flex items-center space-x-2">
                        {player.is_ai && (
                          <span className="px-3 py-1 text-sm bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200 rounded-full">
                            AI
                          </span>
                        )}
                        {gameData?.game.state.toLowerCase() === 'waiting' ? (
                          player.is_ready ? (
                            <span className="px-3 py-1 text-sm bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200 rounded-full">
                              Ready
                            </span>
                          ) : (
                            <span className="px-3 py-1 text-sm bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200 rounded-full">
                              Not Ready
                            </span>
                          )
                        ) : (
                          player.turn_order !== null && (
                            <span className={`px-2 py-1 text-xs rounded ${
                              isCurrentTurn 
                                ? 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200 font-bold' 
                                : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300'
                            }`}>
                              Turn {player.turn_order + 1}
                            </span>
                          )
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* Game Status */}
        {gameData && gameData.game.state.toLowerCase() === 'waiting' && (
          <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4 mt-6">
            <div className="flex items-center">
              <svg className="w-5 h-5 text-blue-400 mr-2" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clipRule="evenodd" />
              </svg>
              <span className="text-blue-800 dark:text-blue-200 font-medium">Waiting for players</span>
            </div>
            <p className="text-blue-700 dark:text-blue-300 mt-1">
              The game will start when all players are ready.
            </p>
          </div>
        )}

        {gameData && gameData.game.state.toLowerCase() === 'started' && (
          <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-4 mt-6">
            <div className="flex items-center">
              <svg className="w-5 h-5 text-green-400 mr-2" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
              </svg>
              <span className="text-green-800 dark:text-green-200 font-medium">Game in progress</span>
            </div>
            <p className="text-green-700 dark:text-green-300 mt-1">
              The game has started! Game interface will be implemented here.
            </p>
            {gameState && gameState.game.current_turn !== null && (
              <div className="mt-3 p-3 bg-white dark:bg-gray-800 rounded-lg border">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                    Current Turn: {gameState.game.current_turn + 1}
                  </span>
                  {gameStatePollingInterval && (
                    <span className="text-xs text-green-600 dark:text-green-400">
                      • Auto-updating every 2.5s
                    </span>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
} 