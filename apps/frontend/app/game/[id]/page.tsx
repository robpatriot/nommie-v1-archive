'use client';

import { useState, useCallback } from 'react';
import { useParams } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { useRouter } from 'next/navigation';
import { BACKEND_URL } from '@/lib/config';
import { usePolling } from '@/hooks/usePolling';
import PlayerHand from '@/components/PlayerHand';
import BiddingInterface from '@/components/BiddingInterface';

interface PlayerSnapshot {
  id: string;
  user_id: string;
  turn_order: number | null;
  is_ready: boolean;
  is_ai: boolean;
  total_score: number;
  hand: string[] | null;
  user: {
    id: string;
    email: string;
    name: string | null;
  };
}

interface GameInfo {
  id: string;
  state: string;
  phase: string;
  current_turn: number | null;
  created_at: string;
  updated_at: string;
  started_at?: string;
}

interface RoundBidSnapshot {
  player_id: string;
  bid: number;
}

interface RoundSnapshot {
  id: string;
  round_number: number;
  phase: string;
  dealer_player_id: string | null;
  trump_suit: string | null;
  cards_dealt: number;
  bids: RoundBidSnapshot[];
  current_bidder_turn: number | null;
  current_trick: any | null;
  completed_tricks: any[];
  current_player_turn: string | null;
  round_scores: any[];
}

interface GameSnapshot {
  game: GameInfo;
  players: PlayerSnapshot[];
  current_round: RoundSnapshot | null;
  player_count: number;
  max_players: number;
  trump_chooser_id: string | null;
}

export default function GamePage() {
  const params = useParams();
  const { data: session, status } = useSession();
  const router = useRouter();
  const gameId = params.id as string;

  const [gameSnapshot, setGameSnapshot] = useState<GameSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [markingReady, setMarkingReady] = useState(false);
  const [addingAI, setAddingAI] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [submittingTrump, setSubmittingTrump] = useState(false);
  const [selectedTrumpSuit, setSelectedTrumpSuit] = useState<string>('');

  const fetchGameData = useCallback(
    async (isManualRefresh = false) => {
      if (isManualRefresh) {
        setRefreshing(true);
      } else {
        setLoading(true);
      }
      setError(null);

      try {
        const session = await import('next-auth/react').then((m) => m.getSession());

        if (!session?.accessToken) {
          throw new Error('No access token available');
        }

        const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/state`, {
          headers: {
            Authorization: `Bearer ${session.accessToken}`,
            'Content-Type': 'application/json',
          },
        });

        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`HTTP ${response.status}: ${errorText}`);
        }

        const data: GameSnapshot = await response.json();
        setGameSnapshot(data);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to fetch game data');
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [gameId],
  );

  // Use the polling hook
  const { isPolling } = usePolling({
    enabled: status === 'authenticated' && !!gameId,
    interval: 2500, // 2.5 seconds
    callback: fetchGameData,
    immediate: true,
  });

  const handleManualRefresh = () => {
    fetchGameData(true);
  };

  const handleMarkReady = async () => {
    setMarkingReady(true);
    setError(null);
    setSuccessMessage(null);

    try {
      const session = await import('next-auth/react').then((m) => m.getSession());

      if (!session?.accessToken) {
        throw new Error('No access token available');
      }

      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/ready`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${session.accessToken}`,
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
        if (gameSnapshot) {
          setGameSnapshot({
            ...gameSnapshot,
            players: gameSnapshot.players.map((player: PlayerSnapshot) =>
              player.user.email === session.user?.email ? { ...player, is_ready: true } : player,
            ),
            // Update game state if the game started
            game: data.game_started
              ? {
                  ...gameSnapshot.game,
                  state: 'Started',
                  started_at: new Date().toISOString(),
                }
              : gameSnapshot.game,
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
      const session = await import('next-auth/react').then((m) => m.getSession());

      if (!session?.accessToken) {
        throw new Error('No access token available');
      }

      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/add_ai`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }

      const data = await response.json();

      if (data.success) {
        setSuccessMessage('AI player added successfully!');
        // Refresh game data to show the new player
        fetchGameData(true);
      } else {
        throw new Error(data.message || 'Failed to add AI player');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add AI player');
    } finally {
      setAddingAI(false);
    }
  };

  const handleSubmitTrump = async (trumpSuit: string) => {
    setSubmittingTrump(true);
    setError(null);
    setSuccessMessage(null);

    try {
      const session = await import('next-auth/react').then((m) => m.getSession());

      if (!session?.accessToken) {
        throw new Error('No access token available');
      }

      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/trump`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${session.accessToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ trump_suit: trumpSuit }),
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }

      const data = await response.json();
      setSuccessMessage(`Trump suit selected: ${trumpSuit}`);

      // Refresh game data to get updated state
      await fetchGameData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to submit trump suit');
    } finally {
      setSubmittingTrump(false);
    }
  };

  const getGameStateColor = (state: string) => {
    switch (state.toLowerCase()) {
      case 'waiting':
        return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200';
      case 'started':
        return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200';
      case 'completed':
        return 'bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-200';
      default:
        return 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200';
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

  const currentPlayer = gameSnapshot?.players.find((p) => p.user.email === session?.user?.email);

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
              {gameSnapshot && (
                <span
                  className={`px-3 py-1 text-sm rounded-full font-medium ${getGameStateColor(gameSnapshot.game.state)}`}
                >
                  {getGameStateText(gameSnapshot.game.state)}
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

          {gameSnapshot && (
            <div className="text-sm text-gray-600 dark:text-gray-400">
              Players: {gameSnapshot.player_count}/{gameSnapshot.max_players}
              {isPolling && (
                <span className="ml-4 text-green-600 dark:text-green-400">
                  • Auto-refreshing every 2.5 seconds
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
                <path
                  fillRule="evenodd"
                  d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1-1z"
                  clipRule="evenodd"
                />
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
                <path
                  fillRule="evenodd"
                  d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                  clipRule="evenodd"
                />
              </svg>
              <span className="text-green-800 font-medium">Success</span>
            </div>
            <p className="text-green-700 mt-1">{successMessage}</p>
          </div>
        )}

        {/* Loading State */}
        {loading && !gameSnapshot && (
          <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
            <div className="text-center py-8">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
              <p className="mt-2 text-gray-600 dark:text-gray-400">Loading game data...</p>
            </div>
          </div>
        )}

        {/* Game Content */}
        {gameSnapshot && (
          <>
            {/* Player Hand */}
            {currentPlayer?.hand && (
              <div className="mb-6">
                <PlayerHand cards={currentPlayer.hand} />
              </div>
            )}

            {/* Bidding Interface */}
            {gameSnapshot.game.phase === 'bidding' && gameSnapshot.current_round && (
              <div className="mb-6">
                <BiddingInterface
                  gameId={gameId}
                  players={gameSnapshot.players}
                  currentRound={gameSnapshot.current_round}
                  onBidSubmitted={fetchGameData}
                />
              </div>
            )}

            {/* Trump Selection UI */}
            {gameSnapshot.game.phase === 'trump_selection' && (
              <div className="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg p-6 mt-6">
                <div className="flex items-center mb-4">
                  <svg
                    className="w-5 h-5 text-yellow-400 mr-2"
                    fill="currentColor"
                    viewBox="0 0 20 20"
                  >
                    <path
                      fillRule="evenodd"
                      d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z"
                      clipRule="evenodd"
                    />
                  </svg>
                  <span className="text-yellow-800 dark:text-yellow-200 font-medium">
                    Trump Selection Phase
                  </span>
                </div>

                {gameSnapshot.current_round?.trump_suit ? (
                  <div className="text-yellow-700 dark:text-yellow-300">
                    <p className="font-medium">
                      Trump suit has been selected:{' '}
                      <span className="font-bold">{gameSnapshot.current_round.trump_suit}</span>
                    </p>
                    <p className="text-sm mt-1">
                      Waiting for the game to transition to playing phase...
                    </p>
                  </div>
                ) : (
                  <div>
                    {(() => {
                      // Check if current user is the trump chooser
                      const currentUser = gameSnapshot.players.find(
                        (p) => p.user.email === session?.user?.email,
                      );
                      const isTrumpChooser =
                        currentUser && gameSnapshot.trump_chooser_id === currentUser.id;

                      if (isTrumpChooser) {
                        return (
                          <div>
                            <p className="text-yellow-700 dark:text-yellow-300 mb-4">
                              You are the highest bidder! Please select a trump suit for this round.
                            </p>
                            <div className="space-y-3">
                              <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
                                {['Spades', 'Hearts', 'Diamonds', 'Clubs', 'NoTrump'].map(
                                  (suit) => (
                                    <button
                                      key={suit}
                                      onClick={() => setSelectedTrumpSuit(suit)}
                                      className={`px-4 py-3 rounded-lg border-2 font-medium transition-colors ${
                                        selectedTrumpSuit === suit
                                          ? 'border-blue-500 bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200'
                                          : 'border-gray-300 dark:border-gray-600 hover:border-gray-400 dark:hover:border-gray-500 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300'
                                      }`}
                                    >
                                      {suit === 'NoTrump' ? 'No Trump' : suit}
                                    </button>
                                  ),
                                )}
                              </div>
                              {selectedTrumpSuit && (
                                <div className="flex justify-center">
                                  <button
                                    onClick={() => handleSubmitTrump(selectedTrumpSuit)}
                                    disabled={submittingTrump}
                                    className="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
                                  >
                                    {submittingTrump ? (
                                      <div className="flex items-center">
                                        <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                                        Submitting...
                                      </div>
                                    ) : (
                                      `Submit ${selectedTrumpSuit === 'NoTrump' ? 'No Trump' : selectedTrumpSuit}`
                                    )}
                                  </button>
                                </div>
                              )}
                            </div>
                          </div>
                        );
                      } else {
                        return (
                          <div className="text-yellow-700 dark:text-yellow-300">
                            <p>Waiting for another player to choose trump...</p>
                            {gameSnapshot.trump_chooser_id && (
                              <p className="text-sm mt-1">
                                {(() => {
                                  const trumpChooser = gameSnapshot.players.find(
                                    (p) => p.id === gameSnapshot.trump_chooser_id,
                                  );
                                  return trumpChooser
                                    ? `${trumpChooser.user.name || trumpChooser.user.email} is selecting trump...`
                                    : 'Highest bidder is selecting trump...';
                                })()}
                              </p>
                            )}
                          </div>
                        );
                      }
                    })()}
                  </div>
                )}
              </div>
            )}

            {/* Players List */}
            <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-xl font-semibold text-gray-900 dark:text-white">Players</h2>

                {/* Ready Button */}
                {gameSnapshot.game.state.toLowerCase() === 'waiting' && (
                  <div className="flex space-x-2">
                    <button
                      onClick={handleMarkReady}
                      disabled={
                        markingReady ||
                        gameSnapshot.players.find(
                          (p: PlayerSnapshot) => p.user.email === session?.user?.email,
                        )?.is_ready
                      }
                      className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
                    >
                      {markingReady ? (
                        <div className="flex items-center">
                          <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                          Marking Ready...
                        </div>
                      ) : gameSnapshot.players.find(
                          (p: PlayerSnapshot) => p.user.email === session?.user?.email,
                        )?.is_ready ? (
                        'Ready ✓'
                      ) : (
                        'Mark Ready'
                      )}
                    </button>
                    <button
                      onClick={handleAddAI}
                      disabled={addingAI || gameSnapshot.player_count >= gameSnapshot.max_players}
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

              {gameSnapshot.players.length === 0 ? (
                <div className="text-center py-8">
                  <div className="text-gray-400 dark:text-gray-500 mb-2">
                    <svg
                      className="w-12 h-12 mx-auto"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197m13.5-9a2.5 2.5 0 11-5 0 2.5 2.5 0 015 0z"
                      />
                    </svg>
                  </div>
                  <p className="text-gray-600 dark:text-gray-400">No players in game</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {gameSnapshot.players.map((player: PlayerSnapshot) => {
                    const isCurrentUser = player.user.email === session?.user?.email;
                    const isCurrentTurn =
                      gameSnapshot.game.current_turn !== null &&
                      player.turn_order === gameSnapshot.game.current_turn;

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
                          <div
                            className={`w-10 h-10 rounded-full flex items-center justify-center ${
                              player.is_ai
                                ? 'bg-purple-100 dark:bg-purple-900'
                                : 'bg-blue-100 dark:bg-blue-900'
                            }`}
                          >
                            <span
                              className={`font-medium ${
                                player.is_ai
                                  ? 'text-purple-600 dark:text-purple-400'
                                  : 'text-blue-600 dark:text-blue-400'
                              }`}
                            >
                              {player.is_ai
                                ? '🤖'
                                : (player.user.name || player.user.email).charAt(0).toUpperCase()}
                            </span>
                          </div>
                          <div>
                            <p
                              className={`font-medium text-gray-900 dark:text-white ${
                                isCurrentTurn ? 'font-bold' : ''
                              }`}
                            >
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
                          {gameSnapshot.game.state.toLowerCase() === 'waiting' ? (
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
                              <span
                                className={`px-2 py-1 text-xs rounded ${
                                  isCurrentTurn
                                    ? 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200 font-bold'
                                    : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300'
                                }`}
                              >
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

            {/* Game Status */}
            {gameSnapshot.game.state.toLowerCase() === 'waiting' && (
              <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4 mt-6">
                <div className="flex items-center">
                  <svg
                    className="w-5 h-5 text-blue-400 mr-2"
                    fill="currentColor"
                    viewBox="0 0 20 20"
                  >
                    <path
                      fillRule="evenodd"
                      d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z"
                      clipRule="evenodd"
                    />
                  </svg>
                  <span className="text-blue-800 dark:text-blue-200 font-medium">
                    Waiting for players
                  </span>
                </div>
                <p className="text-blue-700 dark:text-blue-300 mt-1">
                  The game will start when all players are ready.
                </p>
              </div>
            )}

            {gameSnapshot.game.state.toLowerCase() === 'started' && (
              <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-4 mt-6">
                <div className="flex items-center">
                  <svg
                    className="w-5 h-5 text-green-400 mr-2"
                    fill="currentColor"
                    viewBox="0 0 20 20"
                  >
                    <path
                      fillRule="evenodd"
                      d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                      clipRule="evenodd"
                    />
                  </svg>
                  <span className="text-green-800 dark:text-green-200 font-medium">
                    Game in progress
                  </span>
                </div>
                <p className="text-green-700 dark:text-green-300 mt-1">
                  {gameSnapshot.game.phase === 'bidding' &&
                    'Bidding phase - players are placing their bids.'}
                  {gameSnapshot.game.phase === 'trump_selection' &&
                    'Trump selection phase - highest bidder is choosing trump.'}
                  {gameSnapshot.game.phase === 'playing' &&
                    'Playing phase - cards are being played.'}
                  {gameSnapshot.game.phase === 'scoring' &&
                    'Scoring phase - calculating round scores.'}
                </p>
                {gameSnapshot.game.current_turn !== null && (
                  <div className="mt-3 p-3 bg-white dark:bg-gray-800 rounded-lg border">
                    <div className="flex items-center justify-between">
                      <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                        Current Turn: {gameSnapshot.game.current_turn + 1}
                      </span>
                    </div>
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
