'use client';

import { useState } from 'react';
import { useSession } from 'next-auth/react';
import { BACKEND_URL } from '@/lib/config';

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
  current_trick: unknown | null;
  completed_tricks: unknown[];
  current_player_turn: string | null;
  round_scores: unknown[];
}

interface BiddingInterfaceProps {
  gameId: string;
  players: PlayerSnapshot[];
  currentRound: RoundSnapshot;
  onBidSubmitted?: () => void;
}

export default function BiddingInterface({
  gameId,
  players,
  currentRound,
  onBidSubmitted,
}: BiddingInterfaceProps) {
  const { data: session } = useSession();
  const [bidValue, setBidValue] = useState<number>(0);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const currentPlayer = players.find((p) => p.user.email === session?.user?.email);
  const isCurrentPlayerTurn = currentRound.current_player_turn === currentPlayer?.id;
  const currentPlayerBid = currentRound.bids.find((b) => b.player_id === currentPlayer?.id);

  const handleSubmitBid = async () => {
    if (bidValue < 0 || bidValue > 13) {
      setError('Bid must be between 0 and 13');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      const response = await fetch(`${BACKEND_URL}/api/game/${gameId}/bid`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${session?.accessToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ bid: bidValue }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.error || 'Failed to submit bid');
      }

      setBidValue(0);
      onBidSubmitted?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to submit bid');
    } finally {
      setIsSubmitting(false);
    }
  };

  const getPlayerBid = (playerId: string) => {
    const bid = currentRound.bids.find((b) => b.player_id === playerId);
    return bid ? bid.bid : null;
  };

  const isPlayerTurn = (playerId: string) => {
    return currentRound.current_player_turn === playerId;
  };

  return (
    <div className="space-y-6">
      {/* Bidding Status */}
      <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold text-blue-900 dark:text-blue-100 mb-2">
          Bidding Phase - Round {currentRound.round_number}
        </h3>
        <p className="text-blue-700 dark:text-blue-300">
          {isCurrentPlayerTurn ? 'Your turn to bid!' : 'Waiting for other players to bid...'}
        </p>
      </div>

      {/* Player Bids */}
      <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">Player Bids</h3>
        <div className="space-y-3">
          {players.map((player) => {
            const bid = getPlayerBid(player.id);
            const isTurn = isPlayerTurn(player.id);
            const isCurrentUser = player.user.email === session?.user?.email;

            return (
              <div
                key={player.id}
                className={`flex items-center justify-between p-3 border rounded-lg ${
                  isCurrentUser
                    ? 'border-blue-300 bg-blue-50 dark:border-blue-600 dark:bg-blue-900/20'
                    : isTurn
                      ? 'border-orange-300 bg-orange-50 dark:border-orange-600 dark:bg-orange-900/20'
                      : 'border-gray-200 dark:border-gray-700'
                }`}
              >
                <div className="flex items-center space-x-3">
                  <div
                    className={`w-8 h-8 rounded-full flex items-center justify-center ${
                      player.is_ai
                        ? 'bg-purple-100 dark:bg-purple-900'
                        : 'bg-blue-100 dark:bg-blue-900'
                    }`}
                  >
                    <span
                      className={`text-sm font-medium ${
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
                        isTurn ? 'font-bold' : ''
                      }`}
                    >
                      {player.user.name || player.user.email}
                      {isTurn && (
                        <span className="ml-2 inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200">
                          Current Turn
                        </span>
                      )}
                    </p>
                  </div>
                </div>

                <div className="flex items-center space-x-2">
                  {bid !== null ? (
                    <span className="px-3 py-1 text-sm bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200 rounded-full font-medium">
                      Bid: {bid}
                    </span>
                  ) : (
                    <span className="px-3 py-1 text-sm bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300 rounded-full">
                      —
                    </span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Bid Input (only show for current player's turn) */}
      {isCurrentPlayerTurn && currentPlayerBid === undefined && (
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Submit Your Bid
          </h3>

          {error && (
            <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg">
              <p className="text-red-700 text-sm">{error}</p>
            </div>
          )}

          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-2">
              <label
                htmlFor="bid-input"
                className="text-sm font-medium text-gray-700 dark:text-gray-300"
              >
                Bid (0-13):
              </label>
              <input
                id="bid-input"
                type="number"
                min="0"
                max="13"
                value={bidValue}
                onChange={(e) => setBidValue(parseInt(e.target.value) || 0)}
                className="w-20 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:border-gray-600 dark:text-white"
              />
            </div>

            <button
              onClick={handleSubmitBid}
              disabled={isSubmitting}
              className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
            >
              {isSubmitting ? (
                <div className="flex items-center">
                  <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                  Submitting...
                </div>
              ) : (
                'Submit Bid'
              )}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
